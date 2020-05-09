//! Core data structures

use crate::errors::*;

use crate::proto;

use ndarray::prelude::Ix1;

use std::collections::{HashMap, BTreeMap};
use ndarray::{ArrayD, arr0};

use crate::utilities::{standardize_categorical_argument, deduplicate};

/// The universal data representation.
///
/// Arguments to components are hash-maps of Value and the result of a component is a Value.
/// The Value is also used in the validator for public arguments.
///
/// The Value has a one-to-one mapping to a protobuf Value.
///
/// Components unwrap arguments into more granular types, like ndarray::Array1<f64>,
/// run a computation, and then repackage the result back into a Value.
#[derive(Clone, Debug)]
pub enum Value {
    /// An arbitrary-dimensional homogeneously typed array
    Array(Array),
    /// A hash-map, where the keys are enum-typed and the values are of type Value
    Hashmap(Hashmap<Value>),
    /// A 2D homogeneously typed matrix, where the columns may be unknown and the column lengths may be inconsistent
    Jagged(Jagged),
}

impl Value {
    /// Retrieve an Array from a Value, assuming the Value contains an Array
    pub fn array(&self) -> Result<&Array> {
        match self {
            Value::Array(array) => Ok(array),
            _ => Err("value must be an Array".into())
        }
    }
    /// Retrieve Jagged from a Value, assuming the Value contains Jagged
    pub fn jagged(&self) -> Result<&Jagged> {
        match self {
            Value::Jagged(jagged) => Ok(jagged),
            _ => Err("value must be Jagged".into())
        }
    }

    /// Retrieve the first f64 from a Value, assuming a Value contains an ArrayND of type f64
    pub fn first_f64(&self) -> Result<f64> {
        match self {
            Value::Array(array) => array.first_f64(),
            _ => Err("cannot retrieve first float".into())
        }
    }
    /// Retrieve the first i64 from a Value, assuming a Value contains an ArrayND of type i64
    pub fn first_i64(&self) -> Result<i64> {
        match self {
            Value::Array(array) => array.first_i64(),
            _ => Err("cannot retrieve integer".into())
        }
    }
    /// Retrieve the first String from a Value, assuming a Value contains an ArrayND of type String
    pub fn first_string(&self) -> Result<String> {
        match self {
            Value::Array(array) => array.first_string(),
            _ => Err("cannot retrieve string".into())
        }
    }
    /// Retrieve the first bool from a Value, assuming a Value contains an ArrayND of type bool
    pub fn first_bool(&self) -> Result<bool> {
        match self {
            Value::Array(array) => array.first_bool(),
            _ => Err("cannot retrieve bool".into())
        }
    }
}


// build Value from other types with .into()
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Array(Array::Bool(arr0(value).into_dyn()))
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Array(Array::F64(arr0(value).into_dyn()))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Array(Array::I64(arr0(value).into_dyn()))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Array(Array::Str(arr0(value).into_dyn()))
    }
}

impl From<ArrayD<bool>> for Value {
    fn from(value: ArrayD<bool>) -> Self {
        Value::Array(Array::Bool(value))
    }
}

impl From<ArrayD<f64>> for Value {
    fn from(value: ArrayD<f64>) -> Self {
        Value::Array(Array::F64(value))
    }
}

impl From<ArrayD<i64>> for Value {
    fn from(value: ArrayD<i64>) -> Self {
        Value::Array(Array::I64(value))
    }
}

impl From<ArrayD<String>> for Value {
    fn from(value: ArrayD<String>) -> Self {
        Value::Array(Array::Str(value))
    }
}

impl From<BTreeMap<bool, Value>> for Value {
    fn from(value: BTreeMap<bool, Value>) -> Self {
        Value::Hashmap(Hashmap::<Value>::Bool(value))
    }
}

impl From<BTreeMap<i64, Value>> for Value {
    fn from(value: BTreeMap<i64, Value>) -> Self {
        Value::Hashmap(Hashmap::<Value>::I64(value))
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(value: BTreeMap<String, Value>) -> Self {
        Value::Hashmap(Hashmap::<Value>::Str(value))
    }
}

impl From<std::num::TryFromIntError> for Error {
    fn from(value: std::num::TryFromIntError) -> Self {
        format!("{}", value).into()
    }
}
impl From<ndarray_stats::errors::MinMaxError> for Error {
    fn from(value: ndarray_stats::errors::MinMaxError) -> Self {
        format!("min-max error: {}", value).into()
    }
}
impl From<ndarray::ShapeError> for Error {
    fn from(value: ndarray::ShapeError) -> Self {
        format!("shape error: {:?}", value).into()
    }
}


/// The universal n-dimensional array representation.
///
/// ndarray ArrayD's are artificially allowed to be 0, 1 or 2-dimensional.
/// The first axis denotes the number rows/observations. The second axis the number of columns.
///
/// The Array has a one-to-one mapping to a protobuf ArrayND.
#[derive(Clone, Debug)]
pub enum Array {
    Bool(ArrayD<bool>),
    I64(ArrayD<i64>),
    F64(ArrayD<f64>),
    Str(ArrayD<String>),
}

impl Array {
    /// Retrieve the f64 ndarray, assuming the data type of the ArrayND is f64
    pub fn f64(&self) -> Result<&ArrayD<f64>> {
        match self {
            Array::F64(x) => Ok(x),
            Array::I64(_) => Err("atomic type: expected float, got integer".into()),
            Array::Bool(_) => Err("atomic type: expected float, got bool".into()),
            Array::Str(_) => Err("atomic type: expected float, got string".into()),
        }
    }
    pub fn first_f64(&self) -> Result<f64> {
        match self {
            Array::Bool(x) => {
                if x.len() != 1 {
                    return Err("non-singleton array passed for an argument that must be scalar".into());
                }
                Ok(if *x.first().unwrap() { 1. } else { 0. })
            }
            Array::I64(x) => {
                if x.len() != 1 {
                    return Err("non-singleton array passed for an argument that must be scalar".into());
                }
                Ok(f64::from(*x.first().unwrap() as i32))
            }
            Array::F64(x) => {
                if x.len() != 1 {
                    return Err("non-singleton array passed for an argument that must be scalar".into());
                }
                Ok(x.first().unwrap().to_owned())
            }
            _ => Err("value must be numeric".into())
        }
    }
    pub fn vec_f64(&self, optional_length: Option<i64>) -> Result<Vec<f64>> {
        let data = self.f64()?;
        let err_msg = "failed attempt to cast f64 ArrayD to vector".into();
        match data.ndim() {
            0 => match (optional_length, data.first()) {
                (Some(length), Some(v)) => Ok((0..length).map(|_| *v).collect()),
                _ => Err(err_msg)
            },
            1 => Ok(data.clone().into_dimensionality::<Ix1>()?.to_vec()),
            _ => Err(err_msg)
        }
    }
    /// Retrieve the i64 ndarray, assuming the data type of the ArrayND is i64
    pub fn i64(&self) -> Result<&ArrayD<i64>> {
        match self {
            Array::I64(x) => Ok(x),
            Array::F64(_) => Err("atomic type: expected integer, got float".into()),
            Array::Bool(_) => Err("atomic type: expected integer, got bool".into()),
            Array::Str(_) => Err("atomic type: expected integer, got string".into()),
        }
    }
    pub fn first_i64(&self) -> Result<i64> {
        match self {
            Array::Bool(x) => {
                if x.len() != 1 {
                    return Err("non-singleton array passed for an argument that must be scalar".into());
                }
                Ok(if *x.first().unwrap() { 1 } else { 0 })
            }
            Array::I64(x) => {
                if x.len() != 1 {
                    return Err("non-singleton array passed for an argument that must be scalar".into());
                }
                Ok(x.first().unwrap().to_owned())
            }
            _ => Err("value must be numeric".into())
        }
    }
    pub fn vec_i64(&self, optional_length: Option<i64>) -> Result<Vec<i64>> {
        let data = self.i64()?;
        let err_msg = "failed attempt to cast i64 ArrayD to vector".into();
        match data.ndim() {
            0 => match (optional_length, data.first()) {
                (Some(length), Some(v)) => Ok((0..length).map(|_| *v).collect()),
                _ => Err(err_msg)
            },
            1 => Ok(data.clone().into_dimensionality::<Ix1>()?.to_vec()),
            _ => Err(err_msg)
        }
    }
    /// Retrieve the String ndarray, assuming the data type of the ArrayND is String
    pub fn string(&self) -> Result<&ArrayD<String>> {
        match self {
            Array::Str(x) => Ok(x),
            Array::I64(_) => Err("atomic type: expected string, got integer".into()),
            Array::Bool(_) => Err("atomic type: expected string, got bool".into()),
            Array::F64(_) => Err("atomic type: expected string, got float".into()),
        }
    }
    pub fn first_string(&self) -> Result<String> {
        match self {
            Array::Str(x) => {
                if x.len() != 1 {
                    return Err("non-singleton array passed for an argument that must be scalar".into());
                }
                Ok(x.first().unwrap().to_owned())
            }
            _ => Err("value must be a string".into())
        }
    }
    /// Retrieve the bool ndarray, assuming the data type of the ArrayND is bool
    pub fn bool(&self) -> Result<&ArrayD<bool>> {
        match self {
            Array::Bool(x) => Ok(x),
            Array::I64(_) => Err("atomic type: expected bool, got integer".into()),
            Array::Str(_) => Err("atomic type: expected bool, got string".into()),
            Array::F64(_) => Err("atomic type: expected bool, got float".into()),
        }
    }
    pub fn first_bool(&self) -> Result<bool> {
        match self {
            Array::Bool(x) => {
                if x.len() != 1 {
                    return Err("non-singleton array passed for an argument that must be scalar".into());
                }
                Ok(x.first().unwrap().to_owned())
            }
            _ => Err("value must be a bool".into())
        }
    }

    pub fn shape(&self) -> Vec<i64> {
        match self {
            Array::Bool(array) => array.shape().to_owned(),
            Array::F64(array) => array.shape().to_owned(),
            Array::I64(array) => array.shape().to_owned(),
            Array::Str(array) => array.shape().to_owned()
        }.iter().map(|arr| *arr as i64).collect()
    }
    pub fn num_records(&self) -> Result<i64> {
        let shape = self.shape();
        match shape.len() {
            0 => Ok(1),
            1 | 2 => Ok(shape[0]),
            _ => Err("arrays may have max dimensionality of 2".into())
        }
    }
    pub fn num_columns(&self) -> Result<i64> {
        let shape = self.shape();
        match shape.len() {
            0 => Ok(1),
            1 => Ok(1),
            2 => Ok(shape[1]),
            _ => Err("arrays may have max dimensionality of 2".into())
        }
    }
}

/// The universal jagged array representation.
///
/// Typically used to store categorically clamped values.
/// In practice, use is limited to public categories over multiple columns, and the upper triangular covariance matrix
///
/// Jagged has a one-to-one mapping to a protobuf Vector2DJagged.
#[derive(Clone, Debug)]
pub enum Jagged {
    Bool(Vec<Option<Vec<bool>>>),
    I64(Vec<Option<Vec<i64>>>),
    F64(Vec<Option<Vec<f64>>>),
    Str(Vec<Option<Vec<String>>>),
}

impl Jagged {
    /// Retrieve the f64 jagged matrix, assuming the data type of the jagged matrix is f64, and assuming all columns are defined
    pub fn f64(&self) -> Result<Vec<Vec<f64>>> {
        self.f64_option()?.iter().cloned().collect::<Option<Vec<Vec<f64>>>>()
            .ok_or_else(|| "not all columns are known in float Jagged matrix".into())
    }
    /// Retrieve the f64 jagged matrix, assuming the data type of the jagged matrix is f64
    pub fn f64_option(&self) -> Result<&Vec<Option<Vec<f64>>>> {
        match self {
            Jagged::F64(data) => Ok(data),
            _ => Err("expected float type on a non-float Jagged matrix".into())
        }
    }
    /// Retrieve the i64 jagged matrix, assuming the data type of the jagged matrix is i64
    pub fn i64(&self) -> Result<Vec<Vec<i64>>> {
        match self {
            Jagged::I64(data) => data.iter().cloned().collect::<Option<Vec<Vec<i64>>>>()
                .ok_or_else(|| "not all columns are known in int Jagged matrix".into()),
            _ => Err("expected int type on a non-int Jagged matrix".into())
        }
    }
    /// Retrieve the String jagged matrix, assuming the data type of the jagged matrix is String
    pub fn string(&self) -> Result<Vec<Vec<String>>> {
        match self {
            Jagged::Str(data) => data.iter().cloned().collect::<Option<Vec<Vec<String>>>>()
                .ok_or_else(|| "not all columns are known in string Jagged matrix".into()),
            _ => Err("expected string type on a non-string Jagged matrix".into())
        }
    }
    /// Retrieve the bool jagged matrix, assuming the data type of the jagged matrix is bool
    pub fn bool(&self) -> Result<Vec<Vec<bool>>> {
        match self {
            Jagged::Bool(data) => data.iter().cloned().collect::<Option<Vec<Vec<bool>>>>()
                .ok_or_else(|| "not all columns are known in bool Jagged matrix".into()),
            _ => Err("expected bool type on a non-bool Jagged matrix".into())
        }
    }
    pub fn num_columns(&self) -> i64 {
        match self {
            Jagged::Bool(vector) => vector.len() as i64,
            Jagged::F64(vector) => vector.len() as i64,
            Jagged::I64(vector) => vector.len() as i64,
            Jagged::Str(vector) => vector.len() as i64,
        }
    }
    pub fn lengths_option(&self) -> Vec<Option<i64>> {
        match self {
            Jagged::Bool(value) => value.iter()
                .map(|column| column.as_ref().map(|col| col.len() as i64)).collect(),
            Jagged::F64(value) => value.iter()
                .map(|column| column.as_ref().map(|col| col.len() as i64)).collect(),
            Jagged::I64(value) => value.iter()
                .map(|column| column.as_ref().map(|col| col.len() as i64)).collect(),
            Jagged::Str(value) => value.iter()
                .map(|column| column.as_ref().map(|col| col.len() as i64)).collect()
        }
    }
    pub fn lengths(&self) -> Result<Vec<i64>> {
        self.lengths_option().iter().cloned().collect::<Option<Vec<i64>>>()
            .ok_or_else(|| Error::from("length is not defined for every column"))
    }

    pub fn deduplicate(&self) -> Result<Jagged> {
        match self.to_owned() {
            Jagged::F64(_) =>
                Err("float data may not be categorical".into()),
            Jagged::I64(categories) => Ok(categories.into_iter()
                .map(|cats| cats.map(deduplicate))
                .collect::<Vec<Option<Vec<i64>>>>().into()),
            Jagged::Bool(categories) => Ok(categories.into_iter()
                .map(|cats| cats.map(deduplicate))
                .collect::<Vec<Option<Vec<bool>>>>().into()),
            Jagged::Str(categories) => Ok(categories.into_iter()
                .map(|cats| cats.map(deduplicate))
                .collect::<Vec<Option<Vec<String>>>>().into()),
        }
    }

    pub fn standardize(&self, num_columns: &i64) -> Result<Jagged> {
        match self {
            Jagged::F64(_) =>
                Err("float data may not be categorical".into()),
            Jagged::I64(categories) =>
                Ok(standardize_categorical_argument(categories, &num_columns)?
                    .into_iter().map(Some).collect::<Vec<Option<Vec<i64>>>>().into()),
            Jagged::Bool(categories) =>
                Ok(standardize_categorical_argument(categories, &num_columns)?
                    .into_iter().map(Some).collect::<Vec<Option<Vec<bool>>>>().into()),
            Jagged::Str(categories) =>
                Ok(standardize_categorical_argument(categories, &num_columns)?
                    .into_iter().map(Some).collect::<Vec<Option<Vec<String>>>>().into()),
        }
    }
}


impl From<Vec<Option<Vec<f64>>>> for Jagged {
    fn from(value: Vec<Option<Vec<f64>>>) -> Self {
        Jagged::F64(value)
    }
}
impl From<Vec<Option<Vec<i64>>>> for Jagged {
    fn from(value: Vec<Option<Vec<i64>>>) -> Self {
        Jagged::I64(value)
    }
}
impl From<Vec<Option<Vec<bool>>>> for Jagged {
    fn from(value: Vec<Option<Vec<bool>>>) -> Self {
        Jagged::Bool(value)
    }
}
impl From<Vec<Option<Vec<String>>>> for Jagged {
    fn from(value: Vec<Option<Vec<String>>>) -> Self {
        Jagged::Str(value)
    }
}

/// The universal hash-map representation.
///
/// Used for any component that has multiple outputs.
/// In practice, the only components that can emit multiple outputs are materialize (by columns) and partition (by rows)
///
/// The Hashmap has a one-to-one mapping to a protobuf Hashmap.
#[derive(Clone, Debug)]
pub enum Hashmap<T> {
    Bool(BTreeMap<bool, T>),
    I64(BTreeMap<i64, T>),
    Str(BTreeMap<String, T>),
}

impl<T> Hashmap<T> {
    pub fn keys_length(&self) -> i64 {
        match self {
            Hashmap::Bool(value) => value.keys().len() as i64,
            Hashmap::I64(value) => value.keys().len() as i64,
            Hashmap::Str(value) => value.keys().len() as i64,
        }
    }
    pub fn values(&self) -> Vec<&T> {
        match self {
            Hashmap::Bool(value) => value.values().collect(),
            Hashmap::I64(value) => value.values().collect(),
            Hashmap::Str(value) => value.values().collect(),
        }
    }
    pub fn from_values(&self, values: Vec<T>) -> Hashmap<T> where T: Clone {
        match self {
            Hashmap::Bool(value) => value.keys().cloned()
                .zip(values).collect::<BTreeMap<bool, T>>().into(),
            Hashmap::I64(value) => value.keys().cloned()
                .zip(values).collect::<BTreeMap<i64, T>>().into(),
            Hashmap::Str(value) => value.keys().cloned()
                .zip(values).collect::<BTreeMap<String, T>>().into(),
        }
    }
}

impl<T> From<BTreeMap<i64, T>> for Hashmap<T> {
    fn from(value: BTreeMap<i64, T>) -> Self {
        Hashmap::<T>::I64(value)
    }
}
impl<T> From<BTreeMap<bool, T>> for Hashmap<T> {
    fn from(value: BTreeMap<bool, T>) -> Self {
        Hashmap::<T>::Bool(value)
    }
}
impl<T> From<BTreeMap<String, T>> for Hashmap<T> {
    fn from(value: BTreeMap<String, T>) -> Self {
        Hashmap::<T>::Str(value)
    }
}

/// Derived properties for the universal value.
///
/// The ValueProperties has a one-to-one mapping to a protobuf ValueProperties.
#[derive(Clone, Debug)]
pub enum ValueProperties {
    Hashmap(HashmapProperties),
    Array(ArrayProperties),
    Jagged(JaggedProperties),
}


impl ValueProperties {
    /// Retrieve properties corresponding to an ArrayND, assuming the corresponding data value is actually the ArrayND variant
    pub fn array(&self) -> Result<&ArrayProperties> {
        match self {
            ValueProperties::Array(array) => Ok(array),
            _ => Err("value must be an array".into())
        }
    }
    /// Retrieve properties corresponding to an Hashmap, assuming the corresponding data value is actually the Hashmap variant
    pub fn hashmap(&self) -> Result<&HashmapProperties> {
        match self {
            ValueProperties::Hashmap(value) => Ok(value),
            _ => Err("value must be a hashmap".into())
        }
    }
    /// Retrieve properties corresponding to an Vector2DJagged, assuming the corresponding data value is actually the Vector2DJagged variant
    pub fn jagged(&self) -> Result<&JaggedProperties> {
        match self {
            ValueProperties::Jagged(value) => Ok(value),
            _ => Err("value must be a ragged matrix".into())
        }
    }
}


impl From<ArrayProperties> for ValueProperties {
    fn from(value: ArrayProperties) -> Self {
        ValueProperties::Array(value)
    }
}

impl From<HashmapProperties> for ValueProperties {
    fn from(value: HashmapProperties) -> Self {
        ValueProperties::Hashmap(value)
    }
}

impl From<JaggedProperties> for ValueProperties {
    fn from(value: JaggedProperties) -> Self {
        ValueProperties::Jagged(value)
    }
}


/// Derived properties for the universal Hashmap.
///
/// The HashmapProperties has a one-to-one mapping to a protobuf HashmapProperties.
#[derive(Clone, Debug)]
pub struct HashmapProperties {
    /// global count over all partitions
    pub num_records: Option<i64>,
    /// records within the values of the hashmap come from a partition of the rows
    pub disjoint: bool,
    /// properties for each of the values in the hashmap
    pub properties: Hashmap<ValueProperties>,
    pub columnar: bool,
}

impl HashmapProperties {
    pub fn assert_is_disjoint(&self) -> Result<()> {
        if self.disjoint { Err("partitions must be disjoint".into()) } else { Ok(()) }
    }
    pub fn assert_is_not_columnar(&self) -> Result<()> {
        if self.columnar { Err("partitions must not be columnar".into()) } else { Ok(()) }
    }
    pub fn num_records(&self) -> Result<i64> {
        self.num_records.ok_or_else(|| "number of rows is not defined".into())
    }
}


/// Derived properties for the universal ArrayND.
///
/// The ArrayNDProperties has a one-to-one mapping to a protobuf ArrayNDProperties.
#[derive(Clone, Debug)]
pub struct ArrayProperties {
    /// Defined if the number of records is known statically (set by the resize component)
    pub num_records: Option<i64>,
    pub num_columns: Option<i64>,
    /// true if the data may contain null values
    pub nullity: bool,
    /// set to true by the mechanisms. Acts as a filter on the values in the release
    pub releasable: bool,
    /// amplification of privacy usage by unstable data transformations, or possibility of duplicated records
    pub c_stability: Vec<f64>,
    /// set when data is aggregated, used to help compute sensitivity from the mechanisms
    pub aggregator: Option<AggregatorProperties>,
    /// either min/max or categories
    pub nature: Option<Nature>,
    /// f64, i64, bool, String
    pub data_type: DataType,
    /// index of last Materialize or Filter node, where dataset was created
    /// used to determine if arrays are conformable even when N is not known
    pub dataset_id: Option<i64>,
    /// true if the array may not be length zero
    pub is_not_empty: bool,
    /// number of axes in the array
    pub dimensionality: u32
}


/// Derived properties for the universal Vector2DJagged.
///
/// The Vector2DJagged has a one-to-one mapping to a protobuf Vector2DJagged.
#[derive(Clone, Debug)]
pub struct JaggedProperties {
    pub releasable: bool
}

impl ArrayProperties {
    pub fn lower_f64_option(&self) -> Result<Vec<Option<f64>>> {
        match self.nature.to_owned() {
            Some(value) => match value {
                Nature::Continuous(continuous) => match continuous.lower {
                    Vector1DNull::F64(bound) => Ok(bound),
                    _ => Err("lower must be composed of floats".into())
                },
                _ => Err("lower must be an array".into())
            },
            None => Err("continuous nature for lower is not defined".into())
        }
    }
    pub fn lower_f64(&self) -> Result<Vec<f64>> {
        let bound = self.lower_f64_option()?;
        let value = bound.iter().filter_map(|v| v.to_owned()).collect::<Vec<f64>>();
        if bound.len() == value.len() { Ok(value) } else { Err("not all lower bounds are known".into()) }
    }
    pub fn upper_f64_option(&self) -> Result<Vec<Option<f64>>> {
        match self.nature.to_owned() {
            Some(value) => match value {
                Nature::Continuous(continuous) => match continuous.upper {
                    Vector1DNull::F64(bound) => Ok(bound),
                    _ => Err("upper must be composed of floats".into())
                },
                _ => Err("upper must be an array".into())
            },
            None => Err("continuous nature for upper is not defined".into())
        }
    }
    pub fn upper_f64(&self) -> Result<Vec<f64>> {
        let bound = self.upper_f64_option()?;
        let value = bound.iter().filter_map(|v| v.to_owned()).collect::<Vec<f64>>();
        if bound.len() == value.len() { Ok(value) } else { Err("not all upper bounds are known".into()) }
    }

    pub fn lower_i64_option(&self) -> Result<Vec<Option<i64>>> {
        match self.nature.to_owned() {
            Some(value) => match value {
                Nature::Continuous(continuous) => match continuous.lower {
                    Vector1DNull::I64(bound) => Ok(bound),
                    _ => Err("lower must be composed of integers".into())
                },
                _ => Err("lower must be an array".into())
            },
            None => Err("continuous nature for lower is not defined".into())
        }
    }
    pub fn lower_i64(&self) -> Result<Vec<i64>> {
        let bound = self.lower_i64_option()?;
        let value = bound.iter().filter_map(|v| v.to_owned()).collect::<Vec<i64>>();
        if bound.len() == value.len() { Ok(value) } else { Err("not all lower bounds are known".into()) }
    }
    pub fn upper_i64_option(&self) -> Result<Vec<Option<i64>>> {
        match self.nature.to_owned() {
            Some(value) => match value {
                Nature::Continuous(continuous) => match continuous.upper {
                    Vector1DNull::I64(bound) => Ok(bound),
                    _ => Err("upper must be composed of integers".into())
                },
                _ => Err("upper must be an array".into())
            },
            None => Err("continuous nature for upper is not defined".into())
        }
    }
    pub fn upper_i64(&self) -> Result<Vec<i64>> {
        let bound = self.upper_i64_option()?;
        let value = bound.iter().filter_map(|v| v.to_owned()).collect::<Vec<i64>>();
        if bound.len() == value.len() { Ok(value) } else { Err("not all upper bounds are known".into()) }
    }

    pub fn categories(&self) -> Result<Jagged> {
        match self.nature.to_owned() {
            Some(nature) => match nature {
                Nature::Categorical(nature) => Ok(nature.categories),
                _ => Err("categories is not defined".into())
            },
            None => Err("categorical nature is not defined".into())
        }
    }
    pub fn assert_categorical(&self) -> Result<()> {
        self.categories()?.lengths()?;
        Ok(())
    }
    pub fn assert_non_null(&self) -> Result<()> {
        if self.nullity { Err("data may contain nullity when non-nullity is required".into()) } else { Ok(())}
    }
    pub fn assert_is_not_empty(&self) -> Result<()> {
        if self.is_not_empty { Ok(()) } else { Err("data may be empty when non-emptiness is required".into()) }
    }
    pub fn assert_is_releasable(&self) -> Result<()> {
        if self.releasable { Ok(()) } else { Err("data is not releasable when releasability is required".into()) }
    }
    pub fn num_columns(&self) -> Result<i64> {
        self.num_columns.ok_or_else(|| "number of columns is not defined".into())
    }
    pub fn num_records(&self) -> Result<i64> {
        self.num_records.ok_or_else(|| "number of records is not defined".into())
    }
    pub fn assert_is_not_aggregated(&self) -> Result<()> {
        if self.aggregator.is_some() { Err("aggregated data may not be manipulated".into()) }
        else { Ok(()) }
    }
}

/// Fundamental data types for ArrayNDs and Vector2DJagged Values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataType {
    Bool,
    Str,
    F64,
    I64,
}


/// Properties of an aggregation applied to a Value.
///
/// The component variant is passed forward in the graph until a Mechanism needs sensitivity.
/// Since aggregators implement compute_sensitivity,
/// the compute_sensitivity implemented for whatever aggregator was used earlier in the graph is accessible to the mechanism.
///
/// The AggregatorProperties has a one-to-one mapping to a protobuf AggregatorProperties.
#[derive(Clone, Debug)]
pub struct AggregatorProperties {
    pub component: proto::component::Variant,
    pub properties: HashMap<String, ValueProperties>,
}

#[derive(Clone, Debug)]
pub enum Nature {
    Continuous(NatureContinuous),
    Categorical(NatureCategorical),
}

impl Nature {
    pub fn continuous(&self) -> Result<&NatureContinuous> {
        match self {
            Nature::Continuous(continuous) => Ok(continuous),
            _ => Err("nature is categorical when expecting continuous".into())
        }
    }
    pub fn categorical(&self) -> Result<&NatureCategorical> {
        match self {
            Nature::Categorical(categorical) => Ok(categorical),
            _ => Err("nature is continuous when expecting categorical".into())
        }
    }
}

#[derive(Clone, Debug)]
pub struct NatureCategorical {
    pub categories: Jagged
}

#[derive(Clone, Debug)]
pub struct NatureContinuous {
    pub lower: Vector1DNull,
    pub upper: Vector1DNull,
}

#[derive(Clone, Debug)]
pub enum Vector1DNull {
    Bool(Vec<Option<bool>>),
    I64(Vec<Option<i64>>),
    F64(Vec<Option<f64>>),
    Str(Vec<Option<String>>),
}

impl Vector1DNull {
    /// Retrieve the f64 vec, assuming the data type of the ArrayND is f64
    pub fn f64(&self) -> Result<&Vec<Option<f64>>> {
        match self {
            Vector1DNull::F64(x) => Ok(x),
            _ => Err("expected a float on a non-float Vector1DNull".into())
        }
    }
    /// Retrieve the i64 vec, assuming the data type of the ArrayND is i64
    pub fn i64(&self) -> Result<&Vec<Option<i64>>> {
        match self {
            Vector1DNull::I64(x) => Ok(x),
            _ => Err("expected an integer on a non-integer Vector1DNull".into())
        }
    }
}

#[derive(Clone, Debug)]
pub enum Vector1D {
    Bool(Vec<bool>),
    I64(Vec<i64>),
    F64(Vec<f64>),
    Str(Vec<String>),
}

/// Accepted spaces for sensitivity to be computed within.
pub enum SensitivitySpace {
    /// KNorm(1) is L1, KNorm(2) is L2.
    KNorm(u32),
    /// Infinity norm.
    InfNorm,
    Exponential,
}
/// A release consists of Values for each node id.
pub type Release = HashMap<u32, ReleaseNode>;


#[derive(Debug)]
pub struct ReleaseNode {
    pub value: Value,
    pub privacy_usages: Option<Vec<proto::PrivacyUsage>>,
    pub public: bool
}

impl ReleaseNode {
    pub fn new(value: Value) -> ReleaseNode {
        ReleaseNode {
            value,
            privacy_usages: None,
            public: false
        }
    }
}

// The properties for a node consists of Properties for each of its arguments.
pub type NodeProperties = HashMap<String, ValueProperties>;
