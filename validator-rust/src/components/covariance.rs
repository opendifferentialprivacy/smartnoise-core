use crate::errors::*;


use crate::{proto, base, Warnable, Float};

use crate::components::{Component, Sensitivity};
use crate::base::{IndexKey, Value, NodeProperties, AggregatorProperties, SensitivitySpace, ValueProperties, DataType};
use crate::utilities::prepend;
use ndarray::prelude::*;
use indexmap::map::IndexMap;

impl Component for proto::Covariance {
    fn propagate_property(
        &self,
        _privacy_definition: &Option<proto::PrivacyDefinition>,
        _public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: base::NodeProperties,
        node_id: u32
    ) -> Result<Warnable<ValueProperties>> {
        if properties.contains_key(&IndexKey::from("data")) {
            let mut data_property = properties.get::<IndexKey>(&"data".into())
                .ok_or("data: missing")?.array()
                .map_err(prepend("data:"))?.clone();

            data_property.assert_is_not_empty()?;

            if !data_property.releasable {
                data_property.assert_is_not_aggregated()?;
            }

            let num_columns = data_property.num_columns()?;
            let num_columns = num_columns * (num_columns + 1) / 2;

            data_property.c_stability = data_property.c_stability.iter().enumerate()
                .map(|(i, l_stab)| data_property.c_stability.iter().enumerate()
                    .filter(|(j, _)| i <= *j)
                    .map(|(_, r_stab)| l_stab * r_stab)
                    .collect::<Vec<Float>>())
                .flatten().collect::<Vec<Float>>();

            // save a snapshot of the state when aggregating
            data_property.aggregator = Some(AggregatorProperties {
                component: proto::component::Variant::Covariance(self.clone()),
                properties,
                lipschitz_constants: ndarray::Array::from_shape_vec(
                    vec![1, num_columns as usize],
                    (0..num_columns).map(|_| 1.).collect())?.into_dyn().into()
            });

            data_property.num_records = Some(1);
            data_property.num_columns = Some(num_columns);

            if data_property.data_type != DataType::Float {
                return Err("data: atomic type must be float".into());
            }
            // min/max of data is not known after computing covariance
            data_property.nature = None;
            data_property.dataset_id = Some(node_id as i64);
            Ok(ValueProperties::Array(data_property).into())
        } else if properties.contains_key::<IndexKey>(&"left".into()) && properties.contains_key::<IndexKey>(&"right".into()) {
            let mut left_property = properties.get::<IndexKey>(&"left".into())
                .ok_or("left: missing")?.array()
                .map_err(prepend("left:"))?.clone();

            let right_property = properties.get::<IndexKey>(&"right".into())
                .ok_or("right: missing")?.array()
                .map_err(prepend("right:"))?.clone();


            if left_property.data_type != DataType::Float {
                return Err("left: atomic type must be float".into());
            }
            if right_property.data_type != DataType::Float {
                return Err("right: atomic type must be float".into());
            }
            left_property.assert_is_not_empty()?;
            right_property.assert_is_not_empty()?;

            if !left_property.releasable {
                left_property.assert_is_not_aggregated()?;
            }

            if !right_property.releasable {
                right_property.assert_is_not_aggregated()?;
            }

            let num_columns = left_property.num_columns()? * right_property.num_columns()?;

            left_property.c_stability = left_property.c_stability.iter()
                .zip(right_property.c_stability.iter())
                .map(|(l, r)| l * r).collect();

            // save a snapshot of the state when aggregating
            left_property.aggregator = Some(AggregatorProperties {
                component: proto::component::Variant::Covariance(self.clone()),
                properties,
                lipschitz_constants: ndarray::Array::from_shape_vec(
                    vec![1, num_columns as usize],
                    (0..num_columns).map(|_| 1.).collect())?.into_dyn().into()
            });

            left_property.nature = None;
            left_property.releasable = left_property.releasable && right_property.releasable;

            left_property.num_records = Some(1);
            left_property.num_columns = Some(num_columns);
            left_property.dataset_id = Some(node_id as i64);
            Ok(ValueProperties::Array(left_property).into())
        } else {
            Err("either \"data\" for covariance, or \"left\" and \"right\" for cross-covariance must be supplied".into())
        }
    }
}

impl Sensitivity for proto::Covariance {
    /// Covariance sensitivities [are backed by the the proofs here](https://github.com/opendifferentialprivacy/whitenoise-core/blob/955703e3d80405d175c8f4642597ccdf2c00332a/whitepapers/sensitivities/covariance/covariance.pdf).
    fn compute_sensitivity(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        sensitivity_type: &SensitivitySpace,
    ) -> Result<Value> {
        match sensitivity_type {
            SensitivitySpace::KNorm(k) => {
                let data_n;
                let differences = match (properties.get(&IndexKey::from("data")), properties.get::<IndexKey>(&"left".into()), properties.get::<IndexKey>(&"right".into())) {
                    (Some(data_property), None, None) => {

                        // data: perform checks and prepare parameters
                        let data_property = data_property.array()
                            .map_err(prepend("data:"))?.clone();
                        data_property.assert_is_not_aggregated()?;
                        data_property.assert_non_null()?;
                        let data_lower = data_property.lower_float()?;
                        let data_upper = data_property.upper_float()?;
                        data_n = data_property.num_records()? as f64;

                        // collect bound differences for upper triangle of matrix
                        data_lower.iter().zip(data_upper.iter()).enumerate()
                            .map(|(i, (left_min, left_max))|
                                data_lower.iter().zip(data_upper.iter()).enumerate()
                                    .filter(|(j, _)| i <= *j)
                                    .map(|(_, (right_min, right_max))|
                                        (*left_max - *left_min) * (*right_max - *right_min))
                                    .collect::<Vec<Float>>()).flatten().collect::<Vec<Float>>()
                    }
                    (None, Some(left_property), Some(right_property)) => {

                        // left side: perform checks and prepare parameters
                        let left_property = left_property.array()
                            .map_err(prepend("left:"))?.clone();
                        left_property.assert_is_not_aggregated()?;
                        left_property.assert_non_null()?;
                        let left_n = left_property.num_records()?;
                        let left_lower = left_property.lower_float()?;
                        let left_upper = left_property.upper_float()?;

                        // right side: perform checks and prepare parameters
                        let right_property = right_property.array()
                            .map_err(prepend("right:"))?.clone();
                        right_property.assert_is_not_aggregated()?;
                        right_property.assert_non_null()?;
                        let right_n = right_property.num_records()?;
                        let right_lower = right_property.lower_float()?;
                        let right_upper = right_property.upper_float()?;

                        // ensure conformability
                        if left_n != right_n {
                            return Err("n for left and right must be equivalent".into());
                        }
                        data_n = left_n as f64;

                        // collect bound differences for entire matrix
                        left_lower.iter().zip(left_upper.iter())
                            .map(|(left_min, left_max)|
                                right_lower.iter().zip(right_upper.iter())
                                .map(|(right_min, right_max)|
                                    (left_max - *left_min) * (right_max - *right_min))
                                .collect::<Vec<Float>>())
                            .flatten().collect::<Vec<Float>>()
                    }
                    _ => return Err("either \"data\" or \"left\" and \"right\" must be supplied".into())
                };

                let delta_degrees_of_freedom = if self.finite_sample_correction { 1 } else { 0 } as f64;
                let normalization = data_n - delta_degrees_of_freedom;

                use proto::privacy_definition::Neighboring;
                let neighboring_type = Neighboring::from_i32(privacy_definition.neighboring)
                    .ok_or_else(|| Error::from("neighboring definition must be either \"AddRemove\" or \"Substitute\""))?;

                let scaling_constant = match k {
                    1 | 2 => match neighboring_type {
                        Neighboring::AddRemove => data_n / (data_n + 1.) / normalization,
                        Neighboring::Substitute => 2. * (data_n - 1.) / data_n / normalization
                    },
                    _ => return Err("KNorm sensitivity is only supported in L1 and L2 spaces".into())
                } as Float;

                let row_sensitivity = differences.iter()
                    .map(|difference| (difference * scaling_constant))
                    .collect::<Vec<Float>>();

                let mut array_sensitivity = Array::from(row_sensitivity).into_dyn();
                array_sensitivity.insert_axis_inplace(Axis(0));

                Ok(array_sensitivity.into())
            }
            _ => Err("Covariance sensitivity is only implemented for KNorm".into())
        }
    }
}