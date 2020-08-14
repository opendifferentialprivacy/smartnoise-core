use crate::errors::*;

use crate::{proto, base, Warnable, Float};

use crate::components::{Component, Sensitivity, Expandable};
use crate::base::{Value, NodeProperties, AggregatorProperties, SensitivitySpace, ValueProperties, DataType, JaggedProperties, IndexKey};

use crate::utilities::prepend;
use ndarray::prelude::*;
use indexmap::map::IndexMap;


impl Component for proto::Quantile {
    fn propagate_property(
        &self,
        _privacy_definition: &Option<proto::PrivacyDefinition>,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: base::NodeProperties,
        _node_id: u32
    ) -> Result<Warnable<ValueProperties>> {
        let mut data_property = properties.get::<IndexKey>(&"data".into())
            .ok_or("data: missing")?.array()
            .map_err(prepend("data:"))?.clone();

        if !data_property.releasable {
            data_property.assert_is_not_aggregated()?;
        }
        data_property.assert_is_not_empty()?;

        if data_property.data_type != DataType::Float && data_property.data_type != DataType::Int {
            return Err("data: atomic type must be numeric".into());
        }

        Ok(match public_arguments.get::<IndexKey>(&"candidates".into()) {
            Some(candidates) => {
                let candidates = candidates.ref_jagged()?;

                if data_property.data_type != candidates.data_type() {
                    return Err("data_type of data must match data_type of candidates".into())
                }

                let num_columns = data_property.num_columns()?;
                ValueProperties::Jagged(JaggedProperties {
                    num_records: Some(candidates.num_records()),
                    nullity: false,
                    aggregator: Some(AggregatorProperties {
                        component: proto::component::Variant::Quantile(self.clone()),
                        properties,
                        lipschitz_constants: ndarray::Array::from_shape_vec(
                            vec![1, num_columns as usize],
                            (0..num_columns).map(|_| 1.).collect())?.into_dyn().into()
                    }),
                    nature: None,
                    data_type: DataType::Float,
                    releasable: false
                }).into()
            },
            None => {
                let num_columns = data_property.num_columns()?;
                // save a snapshot of the state when aggregating
                data_property.aggregator = Some(AggregatorProperties {
                    component: proto::component::Variant::Quantile(self.clone()),
                    properties,
                    lipschitz_constants: ndarray::Array::from_shape_vec(
                        vec![1, num_columns as usize],
                        (0..num_columns).map(|_| 1.).collect())?.into_dyn().into()
                });

                data_property.num_records = Some(1);
                data_property.nature = None;

                ValueProperties::Array(data_property).into()
            }
        })
    }
}

impl Sensitivity for proto::Quantile {
    fn compute_sensitivity(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        sensitivity_type: &SensitivitySpace,
    ) -> Result<Value> {
        let data_property = properties.get::<IndexKey>(&"data".into())
            .ok_or("data: missing")?.array()
            .map_err(prepend("data:"))?.clone();

        data_property.assert_is_not_aggregated()?;
        data_property.assert_non_null()?;

        match sensitivity_type {
            SensitivitySpace::KNorm(k) => {
                if k != &1 {
                    return Err("Quantile sensitivity is only implemented for KNorm of 1".into());
                }
                let lower = data_property.lower_float()?;
                let upper = data_property.upper_float()?;

                let row_sensitivity = lower.iter()
                    .zip(upper.iter())
                    .map(|(min, max)| max - min)
                    .collect::<Vec<Float>>();

                let mut array_sensitivity = Array::from(row_sensitivity).into_dyn();
                array_sensitivity.insert_axis_inplace(Axis(0));

                Ok(array_sensitivity.into())
            }
            SensitivitySpace::Exponential => {

                let neighboring_type = Neighboring::from_i32(privacy_definition.neighboring)
                    .ok_or_else(|| Error::from("neighboring definition must be either \"AddRemove\" or \"Substitute\""))?;
                use proto::privacy_definition::Neighboring;
                let cell_sensitivity = match neighboring_type {
                    Neighboring::AddRemove => self.alpha.max(1. - self.alpha),
                    Neighboring::Substitute => 1.
                } as Float;

                let row_sensitivity = (0..data_property.num_columns()?)
                    .map(|_| cell_sensitivity)
                    .collect::<Vec<Float>>();

                let array_sensitivity = Array::from(row_sensitivity).into_dyn();
                // array_sensitivity.insert_axis_inplace(Axis(0));

                Ok(array_sensitivity.into())
            }
            _ => Err("Quantile sensitivity is not implemented for the specified sensitivity space".into())
        }
    }
}


macro_rules! make_quantile {
    ($variant:ident, $alpha:expr, $interpolation:expr) => {

        impl Expandable for proto::$variant {
            fn expand_component(
                &self,
                _privacy_definition: &Option<proto::PrivacyDefinition>,
                component: &proto::Component,
                _public_arguments: &IndexMap<IndexKey, &Value>,
                _properties: &base::NodeProperties,
                component_id: u32,
                _maximum_id: u32,
            ) -> Result<base::ComponentExpansion> {
                let mut expansion = base::ComponentExpansion::default();

                expansion.computation_graph.insert(component_id, proto::Component {
                    arguments: component.arguments.clone(),
                    variant: Some(proto::component::Variant::Quantile(proto::Quantile {
                        alpha: $alpha,
                        interpolation: $interpolation
                    })),
                    omit: component.omit,
                    submission: component.submission,
                });
                expansion.traversal.push(component_id);

                Ok(expansion)
            }
        }
    }
}

make_quantile!(Minimum, 0.0, "lower".to_string());
make_quantile!(Median, 0.5, "midpoint".to_string());
make_quantile!(Maximum, 1.0, "upper".to_string());
