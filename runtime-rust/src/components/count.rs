use whitenoise_validator::errors::*;

use crate::NodeArguments;
use whitenoise_validator::base::{Value, Array, ReleaseNode};
use crate::components::Evaluable;
use ndarray::{ArrayD, Axis, arr0};
use whitenoise_validator::{proto, Integer};
use whitenoise_validator::utilities::take_argument;
use std::collections::HashSet;
use crate::utilities::get_num_columns;
use std::iter::FromIterator;
use std::hash::Hash;
use noisy_float::types::n64;


impl Evaluable for proto::Count {
    fn evaluate(&self, _privacy_definition: &Option<proto::PrivacyDefinition>, mut arguments: NodeArguments) -> Result<ReleaseNode> {
        Ok(ReleaseNode::new(if self.distinct {
            match take_argument(&mut arguments, "data")?.array()? {
                Array::Bool(data) => count_distinct(&data)?.into(),
                Array::Float(data) => count_distinct(&data.mapv(|v| n64(v as f64)))?.into(),
                Array::Int(data) => count_distinct(&data)?.into(),
                Array::Str(data) => count_distinct(&data)?.into()
            }
        } else {
            match take_argument(&mut arguments, "data")? {
                Value::Array(array) => match array {
                    Array::Bool(data) => count(&data)?.into(),
                    Array::Float(data) => count(&data)?.into(),
                    Array::Int(data) => count(&data)?.into(),
                    Array::Str(data) => count(&data)?.into()
                },
                Value::Dataframe(dataframe) => match dataframe.get_index(0) {
                    Some(value) => arr0(value.1.ref_array()?.num_records()? as Integer).into_dyn().into(),
                    None => return Err("indexmap may not be empty".into())
                },
                _ => return Err("Count is only implemented on arrays and dataframes".into())
            }
        }))
    }
}

/// Gets number of rows of data.
///
/// # Arguments
/// * `data` - Data for which you want a count.
///
/// # Return
/// Number of rows in data.
///
/// # Example
/// ```
/// use ndarray::{ArrayD, arr1, arr2};
/// use whitenoise_runtime::components::count::count;
/// let data = arr2(&[ [false, false, true], [true, true, true] ]).into_dyn();
/// let n = count(&data).unwrap();
/// assert!(n.first().unwrap() == &2);
/// ```
pub fn count<T>(data: &ArrayD<T>) -> Result<ArrayD<Integer>> {
    Ok(ndarray::Array::from_shape_vec(vec![], vec![data.len_of(Axis(0)) as Integer])?)
}

/// Gets number of unique values in the data.
///
/// # Arguments
/// * `data` - Data for which you want a distinct count.
///
/// # Return
/// Number of rows in data.
///
/// # Example
/// ```
/// use ndarray::{ArrayD, arr1, arr2};
/// use whitenoise_runtime::components::count::count_distinct;
/// let data = arr2(&[ [false, false, true], [true, false, true] ]).into_dyn();
/// let distinct = count_distinct(&data).unwrap();
/// assert_eq!(distinct, arr2(&[ [2, 1, 1] ]).into_dyn());
/// ```
pub fn count_distinct<T: Eq + Hash>(data: &ArrayD<T>) -> Result<ArrayD<Integer>> {
    let counts = data.gencolumns().into_iter().map(|column| {
        HashSet::<&T>::from_iter(column.iter()).len() as Integer
    }).collect::<Vec<Integer>>();

    // ensure counts are of correct dimension
    let array = match data.ndim() {
        1 => ndarray::Array::from_shape_vec(vec![], counts),
        2 => ndarray::Array::from_shape_vec(vec![1 as usize, get_num_columns(&data)? as usize], counts),
        _ => return Err("invalid data shape for Count".into())
    };

    match array {
        Ok(array) => Ok(array),
        Err(_) => Err("unable to package Count result into an array".into())
    }
}
