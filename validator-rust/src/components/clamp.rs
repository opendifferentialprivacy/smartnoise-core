use crate::errors::*;

use crate::base::{Nature, Vector1DNull, Array, ValueProperties, NatureCategorical, Jagged, DataType};

use crate::{proto, base, Warnable};
use crate::utilities::{prepend, get_literal, standardize_null_target_argument};
use crate::components::{Component, Expandable};

use crate::base::{IndexKey, Value, NatureContinuous};
use indexmap::map::IndexMap;
use crate::utilities::inference::infer_property;


impl Component for proto::Clamp {
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

        let num_columns = data_property.num_columns
            .ok_or("data: number of data columns missing")?;

        if !data_property.releasable {
            data_property.assert_is_not_aggregated()?;
        }

        // handle categorical clamping
        if let Some(categories) = public_arguments.get::<IndexKey>(&"categories".into()) {
            let null = public_arguments.get::<IndexKey>(&"null_value".into())
                .ok_or_else(|| Error::from("null value must be defined when clamping by categories"))?
                .ref_array()?;

            let mut categories = categories.ref_jagged()?.clone();
            match (&mut categories, null) {
                (Jagged::Float(jagged), Array::Float(null)) => {
                    let null_target = standardize_null_target_argument(null.clone(), num_columns)?;
                    jagged.iter_mut().zip(null_target.into_iter())
                        .for_each(|(cats, null)| cats.push(null))
                },
                (Jagged::Int(jagged), Array::Int(null)) => {
                    let null_target = standardize_null_target_argument(null.clone(), num_columns)?;
                    jagged.iter_mut().zip(null_target.into_iter())
                        .for_each(|(cats, null)| cats.push(null))
                },
                (Jagged::Str(jagged), Array::Str(null)) => {
                    let null_target = standardize_null_target_argument(null.clone(), num_columns)?;
                    jagged.iter_mut().zip(null_target.into_iter())
                        .for_each(|(cats, null)| cats.push(null))
                },
                (Jagged::Bool(jagged), Array::Bool(null)) => {
                    let null_target = standardize_null_target_argument(null.clone(), num_columns)?;
                    jagged.iter_mut().zip(null_target.into_iter())
                        .for_each(|(cats, null)| cats.push(null))
                },
                _ => return Err("categories and null_value must be homogeneously typed".into())
            };
            categories = categories.standardize(num_columns)?;
            data_property.nature = Some(Nature::Categorical(NatureCategorical { categories }));

            return Ok(ValueProperties::Array(data_property).into())
        }

        // else handle numerical clamping
        match data_property.data_type {
            DataType::Float => {

                // 1. check public arguments (constant n)
                let mut clamp_lower = match public_arguments.get::<IndexKey>(&"lower".into()) {
                    Some(&lower) => lower.ref_array()?.clone().vec_float(Some(num_columns))
                        .map_err(prepend("lower:"))?,

                    // 2. then private arguments (for example from another clamped column)
                    None => match properties.get::<IndexKey>(&"lower".into()) {
                        Some(lower) => lower.array()?.lower_float()
                            .map_err(prepend("lower:"))?,

                        // 3. then data properties (propagated from prior clamping/min/max)
                        None => data_property
                            .lower_float().map_err(prepend("lower:"))?
                    }
                };

                // 1. check public arguments (constant n)
                let mut clamp_upper = match public_arguments.get::<IndexKey>(&"upper".into()) {
                    Some(&upper) => upper.ref_array()?.clone().vec_float(Some(num_columns))
                        .map_err(prepend("upper:"))?,

                    // 2. then private arguments (for example from another clamped column)
                    None => match properties.get::<IndexKey>(&"upper".into()) {
                        Some(upper) => upper.array()?.upper_float()
                            .map_err(prepend("upper:"))?,

                        // 3. then data properties (propagated from prior clamping/min/max)
                        None => data_property
                            .upper_float().map_err(prepend("upper:"))?
                    }
                };

                if !clamp_lower.iter().zip(clamp_upper.clone()).all(|(low, high)| *low < high) {
                    return Err("lower is greater than upper".into());
                }

                // the actual data bound (if it exists) may be tighter than the clamping parameters
                if let Ok(data_minimum) = data_property.lower_float_option() {
                    clamp_lower = clamp_lower.into_iter().zip(data_minimum)
                        // match on if the actual bound exists for each column, and remain conservative if not
                        .map(|(clamp_lower, optional_data_lower)| match optional_data_lower {
                            Some(data_lower) => clamp_lower.max(data_lower), // tighter data bound is only applied here
                            None => clamp_lower
                        }).collect()
                }
                if let Ok(data_upper) = data_property.upper_float_option() {
                    clamp_upper = clamp_upper.into_iter().zip(data_upper)
                        .map(|(clamp_upper, optional_data_upper)| match optional_data_upper {
                            Some(data_upper) => clamp_upper.min(data_upper),
                            None => clamp_upper
                        }).collect()
                }

                // save revised bounds
                data_property.nature = Some(Nature::Continuous(NatureContinuous {
                    lower: Vector1DNull::Float(clamp_lower.into_iter().map(Some).collect()),
                    upper: Vector1DNull::Float(clamp_upper.into_iter().map(Some).collect()),
                }));

            }

            DataType::Int => {
                // 1. check public arguments (constant n)
                let mut clamp_lower = match public_arguments.get::<IndexKey>(&"lower".into()) {
                    Some(&lower) => lower.ref_array()?.clone().vec_int(Some(num_columns))
                        .map_err(prepend("lower:"))?,

                    // 2. then private arguments (for example from another clamped column)
                    None => match properties.get::<IndexKey>(&"lower".into()) {
                        Some(lower) => lower.array()?.lower_int()
                            .map_err(prepend("lower:"))?,

                        // 3. then data properties (propagated from prior clamping/lower/upper)
                        None => data_property
                            .lower_int().map_err(prepend("lower:"))?
                    }
                };

                // 1. check public arguments (constant n)
                let mut clamp_upper = match public_arguments.get::<IndexKey>(&"upper".into()) {
                    Some(&upper) => upper.ref_array()?.clone().vec_int(Some(num_columns))
                        .map_err(prepend("upper:"))?,

                    // 2. then private arguments (for example from another clamped column)
                    None => match properties.get::<IndexKey>(&"upper".into()) {
                        Some(upper) => upper.array()?.upper_int()
                            .map_err(prepend("upper:"))?,

                        // 3. then data properties (propagated from prior clamping/min/max)
                        None => data_property
                            .upper_int().map_err(prepend("upper:"))?
                    }
                };

                if !clamp_lower.iter().zip(clamp_upper.clone()).all(|(low, high)| *low < high) {
                    return Err("lower is greater than upper".into());
                }

                // the actual data bound (if it exists) may be tighter than the clamping parameters
                if let Ok(data_lower) = data_property.lower_int_option() {
                    clamp_lower = clamp_lower.into_iter().zip(data_lower)
                        // match on if the actual bound exists for each column, and remain conservative if not
                        .map(|(clamp_lower, optional_data_lower)| match optional_data_lower {
                            Some(data_lower) => clamp_lower.max(data_lower), // tighter data bound is only applied here
                            None => clamp_lower
                        }).collect()
                }
                if let Ok(data_upper) = data_property.upper_int_option() {
                    clamp_upper = clamp_upper.into_iter().zip(data_upper)
                        .map(|(clamp_upper, optional_data_upper)| match optional_data_upper {
                            Some(data_upper) => clamp_upper.min(data_upper),
                            None => clamp_upper
                        }).collect()
                }

                // save revised bounds
                data_property.nature = Some(Nature::Continuous(NatureContinuous {
                    lower: Vector1DNull::Int(clamp_lower.into_iter().map(Some).collect()),
                    upper: Vector1DNull::Int(clamp_upper.into_iter().map(Some).collect()),
                }));

            }
            _ => return Err("numeric clamping requires numeric data".into())
        }

        Ok(ValueProperties::Array(data_property).into())
    }

}


impl Expandable for proto::Clamp {
    fn expand_component(
        &self,
        _privacy_definition: &Option<proto::PrivacyDefinition>,
        component: &proto::Component,
        _public_arguments: &IndexMap<IndexKey, &Value>,
        properties: &base::NodeProperties,
        component_id: u32,
        mut maximum_id: u32,
    ) -> Result<base::ComponentExpansion> {
        let mut component = component.clone();

        let mut expansion = base::ComponentExpansion::default();

        let has_categorical = properties.contains_key(&IndexKey::from("categories"));

        if !has_categorical && !properties.contains_key::<IndexKey>(&"lower".into()) {
            maximum_id += 1;
            let id_lower = maximum_id.to_owned();
            let value = Value::Array(Array::Float(
                ndarray::Array::from(properties.get::<IndexKey>(&"data".into()).unwrap().to_owned().array()?.lower_float()?).into_dyn()));
            expansion.properties.insert(id_lower, infer_property(&value, None)?);
            let (patch_node, release) = get_literal(value, component.submission)?;
            expansion.computation_graph.insert(id_lower, patch_node);
            expansion.releases.insert(id_lower, release);
            component.insert_argument(&"lower".into(), id_lower);
        }

        if !has_categorical && !properties.contains_key::<IndexKey>(&"upper".into()) {
            maximum_id += 1;
            let id_upper = maximum_id.to_owned();
            let value = Value::Array(Array::Float(
                ndarray::Array::from(properties.get::<IndexKey>(&"data".into()).unwrap().to_owned().array()?.upper_float()?).into_dyn()));
            expansion.properties.insert(id_upper, infer_property(&value, None)?);
            let (patch_node, release) = get_literal(value, component.submission)?;
            expansion.computation_graph.insert(id_upper, patch_node);
            expansion.releases.insert(id_upper, release);
            component.insert_argument(&"upper".into(), id_upper);
        }

        expansion.computation_graph.insert(component_id, component);

        Ok(expansion)
    }
}


#[cfg(test)]
pub mod test_clamp {
    use crate::base::test_data;

    pub mod utilities {
        use crate::components::cast::test_cast;
        use crate::bindings::Analysis;
        use crate::base::Value;

        pub fn analysis_f64_cont(value: Value, lower: Option<Value>, upper: Option<Value>) -> (Analysis, u32) {
            let (mut analysis, casted) = test_cast::utilities::analysis_f64(value);

            let lower = analysis.literal().value(match lower {
                Some(lower) => lower, None => 0.0.into()
            }).value_public(true).build();
            let upper = analysis.literal().value(match upper {
                Some(upper) => upper, None => 10.0.into()
            }).value_public(true).build();

            let clamped = analysis.clamp(casted)
                .lower(lower).upper(upper)
                .build();

            (analysis, clamped)
        }

        pub fn analysis_i64_cont(value: Value, lower: Option<Value>, upper: Option<Value>) -> (Analysis, u32) {
            let (mut analysis, casted) = test_cast::utilities::analysis_i64(value, lower.clone(), upper.clone());

            let lower = analysis.literal().value(match lower {
                Some(lower) => lower, None => 0.into()
            }).value_public(true).build();
            let upper = analysis.literal().value(match upper {
                Some(upper) => upper, None => 10.into()
            }).value_public(true).build();

            let clamped = analysis.clamp(casted)
                .lower(lower).upper(upper)
                .build();

            (analysis, clamped)
        }

        pub fn analysis_i64_cat(value: Value, categories: Value, null_value: Option<Value>) -> (Analysis, u32) {
            let lower: Value = i64::min_value().into();
            let upper: Value = i64::max_value().into();
            let (mut analysis, casted) = test_cast::utilities::analysis_i64(value, Some(lower), Some(upper));

            let categories = analysis.literal()
                .value(categories).value_public(true)
                .build();

            let null_value = analysis.literal()
                .value(match null_value {
                    Some(null_value) => null_value,
                    None => (-1).into()
                }).value_public(true)
                .build();

            let clamped = analysis.clamp(casted)
                .categories(categories)
                .null_value(null_value)
                .build();

            (analysis, clamped)
        }

        pub fn analysis_string_cat(value: Value, categories: Option<Value>, null_value: Option<Value>) -> (Analysis, u32) {
            let (mut analysis, casted) = test_cast::utilities::analysis_string(value);

            let categories = analysis.literal().value(match categories {
                Some(categories) => categories,
                None => Value::Jagged(vec![vec!["a", "b", "c", "d"].into_iter().map(String::from).collect::<Vec<String>>()].into())
            }).value_public(true).build();

            let null_value = analysis.literal().value(match null_value {
                Some(null_value) => null_value,
                None => "e".to_string().into()
            }).value_public(true).build();

            let clamped = analysis.clamp(casted)
                .categories(categories)
                .null_value(null_value)
                .build();
            (analysis, clamped)
        }

        pub fn analysis_bool_cat(value: Value) -> (Analysis, u32) {
            let (mut analysis, casted) = test_cast::utilities::analysis_bool(value, true.into());
            let categories = analysis.literal()
                .value(Value::Jagged(vec![vec![false, true]].into()))
                .value_public(true).build();

            let null_value = analysis.literal()
                .value(false.into())
                .value_public(true).build();

            let clamped = analysis.clamp(casted)
                .categories(categories)
                .null_value(null_value)
                .build();
            (analysis, clamped)
        }
    }

    macro_rules! test_f64 {
        ( $( $variant:ident; $lower:expr; $upper:expr, )*) => {
            $(
                #[test]
                fn $variant() {
                    let (analysis, clamped) = utilities::analysis_f64_cont(
                        test_data::$variant(), $lower, $upper);
                    analysis.properties(clamped).unwrap();
                }
            )*
        }
    }

    test_f64!(
        array1d_f64_0; Some(0.0.into()); Some(10.0.into()),
        array1d_f64_10_uniform; Some(0.0.into()); Some(10.0.into()),
    );

    macro_rules! test_i64 {
        ( $( $variant:ident; $lower:expr; $upper:expr; $categories:expr; $null_value:expr, )*) => {
            $(
                #[test]
                fn $variant() {
                    use crate::base::Value;

                    let (analysis, clamped) = utilities::analysis_i64_cat(
                        test_data::$variant(),
                        $categories, $null_value);
                    analysis.properties(clamped).unwrap();

                    let (analysis, clamped) = utilities::analysis_i64_cont(
                        test_data::$variant(), $lower, $upper);
                    analysis.properties(clamped).unwrap();
                }
            )*
        }
    }

    test_i64!(
        array1d_i64_0; None; None; Value::Jagged(vec![vec![1]].into()); None,
        array1d_i64_10_uniform; Some(0.into()); Some(10.into()); Value::Jagged(vec![(0..10).collect::<Vec<i64>>()].into()); Some((-1).into()),
    );

    macro_rules! test_string {
        ( $( $variant:ident; $categories:expr; $null_value:expr, )*) => {
            $(
                #[test]
                fn $variant() {
                    let (analysis, clamped) = utilities::analysis_string_cat(
                        test_data::$variant(),
                        $categories, $null_value);
                    analysis.properties(clamped).unwrap();
                }
            )*
        }
    }

    test_string!(
        array1d_string_0; None; None,
        array1d_string_10_uniform; None; None,
    );

    macro_rules! test_bool {
        ( $( $variant:ident, )*) => {
            $(
                #[test]
                fn $variant() {
                    let (analysis, clamped) = utilities::analysis_bool_cat(
                        test_data::$variant());
                    analysis.properties(clamped).unwrap();
                }
            )*
        }
    }

    test_bool!(
        array1d_bool_0,
        array1d_bool_10_uniform,
    );
}
