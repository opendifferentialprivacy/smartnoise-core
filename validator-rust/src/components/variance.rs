use indexmap::map::IndexMap;
use ndarray::prelude::*;

use crate::{base, Float, proto, Warnable};
use crate::base::{
    AggregatorProperties, DataType, IndexKey, Nature, NatureContinuous,
    NodeProperties, SensitivitySpace, Value, ValueProperties, Vector1DNull,
};
use crate::components::{Component, Sensitivity};
use crate::errors::*;
use crate::utilities::prepend;

impl Component for proto::Variance {
    fn propagate_property(
        &self,
        _privacy_definition: &Option<proto::PrivacyDefinition>,
        _public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: base::NodeProperties,
        node_id: u32
    ) -> Result<Warnable<ValueProperties>> {
        let mut data_property = properties.get::<IndexKey>(&"data".into())
            .ok_or("data: missing")?.array()
            .map_err(prepend("data:"))?.clone();

        if !data_property.releasable {
            data_property.assert_is_not_aggregated()?;
        }

        data_property.assert_is_not_empty()?;

        let num_columns = data_property.num_columns()?;
        // save a snapshot of the state when aggregating
        data_property.aggregator = Some(AggregatorProperties::new(
            proto::component::Variant::Variance(self.clone()), properties, num_columns));

        if data_property.data_type != DataType::Float {
            return Err("data: atomic type must be float".into())
        }

        data_property.nature = match (data_property.lower_float(), data_property.upper_float()) {
            (Ok(lower), Ok(upper)) => Some(Nature::Continuous(NatureContinuous {
                lower: Vector1DNull::Float((0..num_columns).map(|_| Some(0.)).collect()),
                upper: Vector1DNull::Float(lower.iter().zip(upper)
                    // Popoviciu's inequality
                    .map(|(l, u)| Some((u - l).powi(2) / 4.)).collect()),
            })),
            _ => None
        };
        data_property.num_records = Some(1);
        data_property.dataset_id = Some(node_id as i64);

        Ok(ValueProperties::Array(data_property).into())
    }
}

impl Sensitivity for proto::Variance {
    /// Variance sensitivities [are backed by the the proofs here](https://github.com/opendp/smartnoise-core/blob/master/whitepapers/sensitivities/variance/variance.pdf)
    fn compute_sensitivity(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        sensitivity_type: &SensitivitySpace
    ) -> Result<Value> {

        match sensitivity_type {
            SensitivitySpace::KNorm(k) => {

                let data_property = properties.get::<IndexKey>(&"data".into())
                    .ok_or("data: missing")?.array()
                    .map_err(prepend("data:"))?.clone();

                data_property.assert_non_null()?;
                data_property.assert_is_not_aggregated()?;
                let data_min = data_property.lower_float()?;
                let data_max = data_property.upper_float()?;
                let data_n = data_property.num_records()? as f64;

                let delta_degrees_of_freedom = if self.finite_sample_correction { 1 } else { 0 } as f64;
                let normalization = data_n - delta_degrees_of_freedom;

                use proto::privacy_definition::Neighboring;
                let neighboring_type = Neighboring::from_i32(privacy_definition.neighboring)
                    .ok_or_else(|| Error::from("neighboring definition must be either \"AddRemove\" or \"Substitute\""))?;

                let scaling_constant = match k {
                    1 | 2 => match neighboring_type {
                        Neighboring::AddRemove => data_n / (data_n + 1.) / normalization,
                        Neighboring::Substitute => (data_n - 1.) / data_n / normalization
                    },
                    _ => return Err("KNorm sensitivity is only supported in L1 and L2 spaces".into())
                } as Float;

                let row_sensitivity = data_min.iter()
                    .zip(data_max.iter())
                    .map(|(min, max)| ((max - min).powi(2) * scaling_constant))
                    .collect::<Vec<Float>>();

                let mut array_sensitivity = Array::from(row_sensitivity).into_dyn();
                array_sensitivity.insert_axis_inplace(Axis(0));

                Ok(array_sensitivity.into())
            },
            _ => Err("Variance sensitivity is only implemented for KNorm of 1".into())
        }
    }
}
