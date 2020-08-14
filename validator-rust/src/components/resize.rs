use crate::errors::*;

use crate::{base, Warnable, Float};
use crate::proto;

use crate::components::{Component, Expandable};

use crate::base::{Value, Array, Nature, NatureContinuous, Vector1DNull, ValueProperties, DataType, IndexKey};
use crate::utilities::{prepend, get_literal};
use indexmap::map::IndexMap;
use crate::utilities::inference::infer_property;


impl Component for proto::Resize {
    fn propagate_property(
        &self,
        _privacy_definition: &Option<proto::PrivacyDefinition>,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: base::NodeProperties,
        _node_id: u32,
    ) -> Result<Warnable<ValueProperties>> {

        let mut data_property = properties.get::<IndexKey>(&"data".into())
            .ok_or("data: missing")?.array()
            .map_err(prepend("data:"))?.clone();

        if !data_property.releasable {
            data_property.assert_is_not_aggregated()?;
        }

        if properties.contains_key::<IndexKey>(&"number_rows".into())
            && properties.contains_key::<IndexKey>(&"minimum_rows".into()) {
            return Err("only one of number_rows and minimum_rows may be set".into())
        }

        if let Some(num_columns) = public_arguments.get::<IndexKey>(&"number_columns".into()) {
            if data_property.num_columns.is_some() {
                return Err("cannot resize number of columns when number of columns is known".into())
            }

            let num_columns = num_columns.ref_array()?.first_int()? as i64;
            if num_columns < 1 {
                return Err("number_columns must be greater than zero".into());
            }
            data_property.num_columns = Some(num_columns);
            data_property.nature = None;
            data_property.c_stability = (0..num_columns).map(|_| 1.).collect::<Vec<Float>>();
            data_property.dimensionality = Some(2);
        }

        if let Some(num_records) = public_arguments.get::<IndexKey>(&"number_rows".into()) {
            let num_records = num_records.ref_array()?.first_int()?;
            if num_records < 1 {
                return Err("number_rows must be greater than zero".into());
            }

            data_property.num_records = Some(num_records as i64);
            data_property.is_not_empty = num_records > 0;
        }

        if let Some(minimum_rows) = public_arguments.get::<IndexKey>(&"minimum_rows".into()) {
            if minimum_rows.ref_array()?.first_int()? > 0 {
                data_property.is_not_empty = true;
            } else {
                return Err("minimum_rows must be greater than zero".into())
            }
        }

        if let Some(categories) = public_arguments.get::<IndexKey>(&"categories".into()) {
            if data_property.data_type != categories.ref_jagged()?.data_type() {
                return Err("data's data_type must match categories' data_type".into());
            }
            // TODO: propagation of categories through imputation and resize
            data_property.nature = None;
            return Ok(ValueProperties::Array(data_property).into())
        }

        let num_columns = data_property.num_columns()?;

        match data_property.data_type {
            DataType::Float => {

                // 1. check public arguments (constant n)
                let impute_lower = match public_arguments.get::<IndexKey>(&"lower".into()) {
                    Some(lower) => lower.ref_array()?.clone().vec_float(Some(num_columns))
                        .map_err(prepend("lower:"))?,

                    // 2. then private arguments (for example from another clamped column)
                    None => match properties.get::<IndexKey>(&"lower".into()) {
                        Some(lower) => lower.array()?.lower_float()
                            .map_err(prepend("lower:"))?,

                        // 3. then data properties (propagated from prior clamping/min/max)
                        None => data_property
                            .lower_float().map_err(prepend("min:"))?
                    }
                };

                // 1. check public arguments (constant n)
                let impute_upper = match public_arguments.get::<IndexKey>(&"upper".into()) {
                    Some(upper) => upper.ref_array()?.clone().vec_float(Some(num_columns))
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

                if !impute_lower.iter().zip(impute_upper.clone()).all(|(low, high)| *low < high) {
                    return Err("lower is greater than upper".into());
                }

                // the actual data bound (if it exists) may be wider than the imputation parameters
                let impute_lower = match data_property.lower_float_option() {
                    Ok(data_lower) => impute_lower.iter().zip(data_lower)
                        .map(|(impute_lower, optional_data_lower)| match optional_data_lower {
                            Some(data_lower) => Some(impute_lower.min(data_lower)),
                            // since there was no prior bound, nothing is known about the min
                            None => None
                        }).collect(),
                    Err(_) => (0..num_columns).map(|_| None).collect()
                };

                let impute_upper = match data_property.upper_float_option() {
                    Ok(data_upper) => impute_upper.iter().zip(data_upper)
                        .map(|(impute_upper, optional_data_upper)| match optional_data_upper {
                            Some(data_upper) => Some(impute_upper.max(data_upper)),
                            // since there was no prior bound, nothing is known about the max
                            None => None
                        }).collect(),
                    Err(_) => (0..num_columns).map(|_| None).collect()
                };

                // impute may only ever widen prior existing bounds
                data_property.nature = Some(Nature::Continuous(NatureContinuous {
                    lower: Vector1DNull::Float(impute_lower),
                    upper: Vector1DNull::Float(impute_upper),
                }));
            }

            DataType::Int => {

                // 1. check public arguments (constant n)
                let impute_lower = match public_arguments.get::<IndexKey>(&"lower".into()) {
                    Some(lower) => lower.ref_array()?.clone().vec_int(Some(num_columns))
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
                let impute_upper = match public_arguments.get::<IndexKey>(&"upper".into()) {
                    Some(upper) => upper.ref_array()?.clone().vec_int(Some(num_columns))
                        .map_err(prepend("upper:"))?,

                    // 2. then private arguments (for example from another clamped column)
                    None => match properties.get::<IndexKey>(&"upper".into()) {
                        Some(upper) => upper.array()?.upper_int()
                            .map_err(prepend("upper:"))?,

                        // 3. then data properties (propagated from prior clamping/lower/upper)
                        None => data_property
                            .upper_int().map_err(prepend("upper:"))?
                    }
                };

                if !impute_lower.iter().zip(impute_upper.clone()).all(|(low, high)| *low < high) {
                    return Err("lower is greater than upper".into());
                }

                // the actual data bound (if it exists) may be wider than the imputation parameters
                let impute_lower = match data_property.lower_int_option() {
                    Ok(data_lower) => impute_lower.into_iter().zip(data_lower.into_iter())
                        .map(|(impute_lower, optional_data_lower)| match optional_data_lower {
                            Some(data_lower) => Some(impute_lower.min(data_lower)),
                            // since there was no prior bound, nothing is known about the min
                            None => None
                        }).collect(),
                    Err(_) => (0..num_columns).map(|_| None).collect()
                };

                let impute_upper = match data_property.upper_int_option() {
                    Ok(data_upper) => impute_upper.into_iter().zip(data_upper.into_iter())
                        .map(|(impute_upper, optional_data_upper)| match optional_data_upper {
                            Some(data_upper) => Some(impute_upper.max(data_upper)),
                            // since there was no prior bound, nothing is known about the max
                            None => None
                        }).collect(),
                    Err(_) => (0..num_columns).map(|_| None).collect()
                };

                // impute may only ever widen prior existing bounds
                data_property.nature = Some(Nature::Continuous(NatureContinuous {
                    lower: Vector1DNull::Int(impute_lower),
                    upper: Vector1DNull::Int(impute_upper),
                }));
            }
            _ => return Err("bounds for imputation must be numeric".into())
        }

        Ok(ValueProperties::Array(data_property).into())
    }
}

impl Expandable for proto::Resize {
    fn expand_component(
        &self,
        _privacy_definition: &Option<proto::PrivacyDefinition>,
        component: &proto::Component,
        _public_arguments: &IndexMap<IndexKey, &Value>,
        properties: &base::NodeProperties,
        component_id: u32,
        mut maximum_id: u32,
    ) -> Result<base::ComponentExpansion> {

        let mut expansion = base::ComponentExpansion::default();

        let mut component = component.clone();

        let data_property = properties.get::<IndexKey>(&"data".into())
            .ok_or("data: missing")?.array()
            .map_err(prepend("data:"))?.clone();

        if !properties.contains_key::<IndexKey>(&"categories".into()) {
            if !properties.contains_key::<IndexKey>(&"lower".into()) {
                maximum_id += 1;
                let id_lower = maximum_id;
                let value = Value::Array(Array::Float(
                    ndarray::Array::from(data_property.lower_float()?).into_dyn()));
                let (patch_node, release) = get_literal(value, component.submission)?;
                expansion.computation_graph.insert(id_lower, patch_node);
                expansion.properties.insert(id_lower, infer_property(&release.value, None)?);
                expansion.releases.insert(id_lower, release);
                component.insert_argument(&"lower".into(), id_lower);
            }

            if !properties.contains_key::<IndexKey>(&"upper".into()) {
                maximum_id += 1;
                let id_upper = maximum_id;
                let value = Value::Array(Array::Float(
                    ndarray::Array::from(data_property.upper_float()?).into_dyn()));
                let (patch_node, release) = get_literal(value, component.submission)?;
                expansion.computation_graph.insert(id_upper, patch_node);
                expansion.properties.insert(id_upper, infer_property(&release.value, None)?);
                expansion.releases.insert(id_upper, release);
                component.insert_argument(&"upper".into(), id_upper);
            }
        }

        expansion.computation_graph.insert(component_id, component);

        Ok(expansion)
    }
}



#[cfg(test)]
pub mod test_resize {
    use crate::base::test_data;

    pub mod utilities {
        use crate::components::impute::test_impute;
        use crate::bindings::Analysis;
        use crate::base::Value;

        pub fn analysis_f64_cont(value: Value, number_rows: Value, lower: Option<Value>, upper: Option<Value>) -> (Analysis, u32) {

            let (mut analysis, imputed) = test_impute::utilities::analysis_f64_cont(
                value,  None, None);

            let lower = analysis.literal().value(match lower {
                Some(lower) => lower, None => 0.0.into()
            }).value_public(true).build();
            let upper = analysis.literal().value(match upper {
                Some(upper) => upper, None => 10.0.into()
            }).value_public(true).build();
            let number_rows = analysis.literal()
                .value(number_rows).value_public(true)
                .build();

            let resized = analysis.resize(imputed)
                .number_rows(number_rows).upper(upper).lower(lower)
                .build();

            (analysis, resized)
        }

        pub fn analysis_i64_cont(value: Value, number_rows: Value, lower: Option<Value>, upper: Option<Value>) -> (Analysis, u32) {
            let (mut analysis, imputed) = test_impute::utilities::analysis_i64_cont(
                value, None, None);

            let lower = analysis.literal().value(match lower {
                Some(lower) => lower, None => 0.into()
            }).value_public(true).build();
            let upper = analysis.literal().value(match upper {
                Some(upper) => upper, None => 10.into()
            }).value_public(true).build();
            let number_rows = analysis.literal()
                .value(number_rows).value_public(true)
                .build();

            let resized = analysis.resize(imputed)
                .number_rows(number_rows).upper(upper).lower(lower)
                .build();

            (analysis, resized)
        }

        pub fn analysis_i64_cat(value: Value, number_rows: Value, categories: Value) -> (Analysis, u32) {
            let (mut analysis, imputed) = test_impute::utilities::analysis_i64_cat(
                value, categories.clone(), None);

            let categories = analysis.literal()
                .value(categories).value_public(true)
                .build();
            let number_rows = analysis.literal()
                .value(number_rows).value_public(true)
                .build();

            let resized = analysis.resize(imputed)
                .number_rows(number_rows)
                .categories(categories)
                .build();

            (analysis, resized)
        }

        pub fn analysis_string_cat(value: Value, number_rows: Value, categories: Option<Value>) -> (Analysis, u32) {
            let (mut analysis, imputed) = test_impute::utilities::analysis_string_cat(
                value, None, None);

            let categories = analysis.literal().value(match categories {
                Some(categories) => categories,
                None => Value::Jagged(vec![vec!["a", "b", "c", "d"].into_iter().map(String::from).collect::<Vec<String>>()].into())
            }).value_public(true).build();
            let number_rows = analysis.literal()
                .value(number_rows).value_public(true)
                .build();

            let resized = analysis.resize(imputed)
                .categories(categories)
                .number_rows(number_rows)
                .build();
            (analysis, resized)
        }

        pub fn analysis_bool_cat(value: Value, number_rows: Value) -> (Analysis, u32) {
            let (mut analysis, imputed) = test_impute::utilities::analysis_bool_cat(value);
            let categories = analysis.literal()
                .value(Value::Jagged(vec![vec![false, true]].into()))
                .value_public(true).build();
            let number_rows = analysis.literal()
                .value(number_rows).value_public(true)
                .build();

            let resized = analysis.resize(imputed)
                .categories(categories)
                .number_rows(number_rows)
                .build();
            (analysis, resized)
        }
    }

    macro_rules! test_f64 {
        ( $( $variant:ident; $number_rows:expr; $lower:expr; $upper:expr, )*) => {
            $(
                #[test]
                fn $variant() {
                    let (analysis, resized) = utilities::analysis_f64_cont(
                        test_data::$variant(), $number_rows, $lower, $upper);
                    analysis.properties(resized).unwrap();
                }
            )*
        }
    }

    test_f64!(
        array1d_f64_0; 10.into(); None; None,
        array1d_f64_10_uniform; 10.into(); None; None,
    );

    macro_rules! test_i64 {
        ( $( $variant:ident; $number_rows:expr; $lower:expr; $upper:expr; $categories:expr; $null_values:expr, )*) => {
            $(
                #[test]
                fn $variant() {
                    use crate::base::Value;

                    let (analysis, resized) = utilities::analysis_i64_cat(
                        test_data::$variant(),
                        $number_rows, $categories);
                    analysis.properties(resized).unwrap();

                    let (analysis, resized) = utilities::analysis_i64_cont(
                        test_data::$variant(), $number_rows, $lower, $upper);
                    analysis.properties(resized).unwrap();
                }
            )*
        }
    }

    test_i64!(
        array1d_i64_0; 10.into(); None; None; Value::Jagged(vec![vec![1]].into()); None,
        array1d_i64_10_uniform; 10.into(); Some(0.into()); Some(10.into()); Value::Jagged(vec![(0..10).collect::<Vec<i64>>()].into()); Some((-1).into()),
    );

    macro_rules! test_string {
        ( $( $variant:ident; $number_rows:expr; $categories:expr; $null_values:expr, )*) => {
            $(
                #[test]
                fn $variant() {
                    let (analysis, resized) = utilities::analysis_string_cat(
                        test_data::$variant(),
                        $number_rows, $categories);
                    analysis.properties(resized).unwrap();
                }
            )*
        }
    }

    test_string!(
        array1d_string_0; 10.into(); None; None,
        array1d_string_10_uniform; 10.into(); None; None,
    );

    macro_rules! test_bool {
        ( $( $variant:ident; $number_rows:expr, )*) => {
            $(
                #[test]
                fn $variant() {
                    let (analysis, resized) = utilities::analysis_bool_cat(
                        test_data::$variant(), $number_rows);
                    analysis.properties(resized).unwrap();
                }
            )*
        }
    }

    test_bool!(
        array1d_bool_0; 10.into(),
        array1d_bool_10_uniform; 10.into(),
    );
}
