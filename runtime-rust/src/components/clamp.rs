use whitenoise_validator::errors::*;

use crate::NodeArguments;
use whitenoise_validator::base::{Value, Array, Jagged, ReleaseNode, IndexKey};
use whitenoise_validator::utilities::{standardize_numeric_argument, standardize_categorical_argument, standardize_null_target_argument, take_argument};
use crate::components::Evaluable;
use ndarray::ArrayD;
use crate::utilities::get_num_columns;
use whitenoise_validator::{proto, Float, Integer};
use std::hash::Hash;

impl Evaluable for proto::Clamp {
    fn evaluate(&self, _privacy_definition: &Option<proto::PrivacyDefinition>, mut arguments: NodeArguments) -> Result<ReleaseNode> {
        // if categories argument was provided, clamp data as if they are categorical (regardless of atomic type)
        if arguments.contains_key::<IndexKey>(&"categories".into()) {
            match (take_argument(&mut arguments, "data")?, take_argument(&mut arguments, "categories")?, take_argument(&mut arguments, "null_value")?) {
                (Value::Array(data), Value::Jagged(categories), Value::Array(nulls)) => Ok(match (data, categories, nulls) {
                    (Array::Bool(data), Jagged::Bool(categories), Array::Bool(nulls)) =>
                        clamp_categorical(data, categories, nulls)?.into(),
                    (Array::Float(_), Jagged::Float(_), Array::Float(_)) =>
                        return Err("float clamping is not supported".into()),
//                        clamp_categorical(&data, &categories, &nulls)?.into(),
                    (Array::Int(data), Jagged::Int(categories), Array::Int(nulls)) =>
                        clamp_categorical(data, categories, nulls)?.into(),
                    (Array::Str(data), Jagged::Str(categories), Array::Str(nulls)) =>
                        clamp_categorical(data, categories, nulls)?.into(),
                    _ => return Err("types of data, categories, and null must be consistent".into())
                }),
                _ => return Err("data must be ArrayND, categories must be Vector2DJagged, and null must be ArrayND".into())
            }
        }
        // if categories argument was not provided, clamp data as numeric
        else {
            match (take_argument(&mut arguments, "data")?, take_argument(&mut arguments, "lower")?, take_argument(&mut arguments, "upper")?) {
                (Value::Array(data), Value::Array(lower), Value::Array(upper)) => Ok(match (data, lower, upper) {
                    (Array::Float(data), Array::Float(lower), Array::Float(upper)) =>
                        clamp_numeric_float(data, lower, upper)?.into(),
                    (Array::Int(data), Array::Int(lower), Array::Int(upper)) =>
                        clamp_numeric_integer(data, lower, upper)?.into(),
                    _ => return Err("data, lower, and upper must all have type f64".into())
                }),
                _ => return Err("data, lower, and upper must all be ArrayND".into())
            }
        }.map(ReleaseNode::new)
    }
}

/// Clamps each column of float data to within desired range.
///
/// # Arguments
/// * `data` - Data to be clamped.
/// * `lower` - Desired lower bound for each column of the data.
/// * `upper` - Desired upper bound for each column of the data.
///
/// # Return
/// Data clamped to desired bounds.
///
/// # Example
/// ```
/// use ndarray::{ArrayD, arr2, arr1};
/// use whitenoise_runtime::components::clamp::clamp_numeric_float;
/// use whitenoise_validator::Float;
/// let data = arr2(&[ [1.,2.,3.], [7.,11.,9.] ]).into_dyn();
/// let lower: ArrayD<Float> = arr1(&[0.5, 8., 4.]).into_dyn();
/// let upper: ArrayD<Float> = arr1(&[2.5, 10., 12.]).into_dyn();
///
/// let clamped_data = clamp_numeric_float(data, lower, upper).unwrap();
/// assert_eq!(clamped_data, arr2(&[ [1., 8., 4.], [2.5, 10., 9.] ]).into_dyn());
/// ```
pub fn clamp_numeric_float(
    mut data: ArrayD<Float>, lower: ArrayD<Float>, upper: ArrayD<Float>
)-> Result<ArrayD<Float>> {

    let num_columns = get_num_columns(&data)?;

    // iterate over the generalized columns
    data.gencolumns_mut().into_iter()
        // pair generalized columns with arguments
        .zip(standardize_numeric_argument(lower, num_columns)?.into_iter())
        .zip(standardize_numeric_argument(upper, num_columns)?.into_iter())
        // for each pairing, iterate over the cells
        .for_each(|((mut column, min), max)| column.iter_mut()
            // ignore nan values
            .filter(|v| !v.is_nan())
            // mutate the cell via the operator
            .for_each(|v| *v = min.max(max.min(*v))));

    Ok(data)
}


/// Clamps each column of integral data to within desired range.
///
/// # Arguments
/// * `data` - Data to be clamped.
/// * `lower` - Desired lower bound for each column of the data.
/// * `upper` - Desired upper bound for each column of the data.
///
/// # Return
/// Data clamped to desired bounds.
///
/// # Example
/// ```
/// use ndarray::{ArrayD, arr2, arr1};
/// use whitenoise_runtime::components::clamp::clamp_numeric_integer;
/// use whitenoise_validator::Integer;
/// let data = arr2(&[ [1, 2, 3], [7, 11, 9] ]).into_dyn();
/// let lower: ArrayD<Integer> = arr1(&[0, 8, 4]).into_dyn();
/// let upper: ArrayD<Integer> = arr1(&[2, 10, 12]).into_dyn();
///
/// let clamped_data = clamp_numeric_integer(data, lower, upper).unwrap();
/// assert_eq!(clamped_data, arr2(&[ [1, 8, 4], [2, 10, 9] ]).into_dyn());
/// ```
pub fn clamp_numeric_integer(
    mut data: ArrayD<Integer>, lower: ArrayD<Integer>, upper: ArrayD<Integer>
)-> Result<ArrayD<Integer>> {

    let num_columns = get_num_columns(&data)?;

    // iterate over the generalized columns
    data.gencolumns_mut().into_iter()
        // pair generalized columns with arguments
        .zip(standardize_numeric_argument(lower, num_columns)?.into_iter())
        .zip(standardize_numeric_argument(upper, num_columns)?.into_iter())
        // for each pairing, iterate over the cells
        .for_each(|((mut column, min), max)| column.iter_mut()
            // mutate the cell via the operator
            .for_each(|v| *v = *min.max(max.min(v))));

    Ok(data)
}

/// Clamps each column of categorical data to desired set.
///
/// Clamping for categorical data is not as obvious a concept as clamping for numeric data.
/// Clamping takes elements not included in `categories` and maps them to the `null_value`.
///
/// This is useful in the library because having a well-defined set of categories (and a default way
/// to refer to elements outside of this set) is important for common procedures like a
/// differentially private histogram release.
///
/// # Arguments
/// * `data` - Data to be clamped.
/// * `categories` - For each column, the set of categories you want to be represented.
/// * `null_value` - For each column, the value to which elements not included in `categories` will be mapped.
///
/// # Return
/// Data clamped to desired bounds.
///
/// # Example
/// ```
/// use ndarray::{ArrayD, arr2, arr1};
/// use whitenoise_runtime::components::clamp::clamp_categorical;
/// let data: ArrayD<String> = arr2(&[["a".to_string(), "b".to_string(), "3".to_string()],
///                                   ["a".to_string(), "2".to_string(), "b".to_string()]]).into_dyn();
/// let categories: Vec<Vec<String>> = vec![vec!["a".to_string(), "b".to_string()],
///                                                 vec!["a".to_string(), "b".to_string()],
///                                                 vec!["a".to_string(), "b".to_string()]];
/// let null_value: ArrayD<String> = arr1(&["not_a_letter".to_string(),
///                                         "not_a_letter".to_string(),
///                                         "not_a_letter".to_string()]).into_dyn();
///
/// let clamped_data = clamp_categorical(data, categories, null_value).unwrap();
/// assert_eq!(clamped_data, arr2(&[["a".to_string(), "b".to_string(), "not_a_letter".to_string()],
///                                ["a".to_string(), "not_a_letter".to_string(), "b".to_string()]]).into_dyn());
/// ```
pub fn clamp_categorical<T: Ord + Hash + Clone>(
    mut data: ArrayD<T>,
    categories: Vec<Vec<T>>,
    null_value: ArrayD<T>
) -> Result<ArrayD<T>> where T:Clone, T:PartialEq, T:Default {

    let num_columns = get_num_columns(&data)?;

    // iterate over the generalized columns
    data.gencolumns_mut().into_iter()
        // pair generalized columns with arguments
        .zip(standardize_categorical_argument(categories.to_vec(), num_columns)?)
        .zip(standardize_null_target_argument(null_value, num_columns)?)
        // for each pairing, iterate over the cells
        .for_each(|((mut column, categories), null)| column.iter_mut()
            // ignore known values
            .filter(|v| !categories.contains(v))
            // mutate the cell via the operator
            .for_each(|v| *v = null.clone()));

    Ok(data)
}
