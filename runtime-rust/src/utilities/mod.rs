pub mod mechanisms;
pub mod noise;

use whitenoise_validator::errors::*;

use openssl::rand::rand_bytes;
use ieee754::Ieee754;

use ndarray::{ArrayD, Zip, Axis};

use rug::Float;
use std::cmp::Ordering;


/// Broadcast left and right to match each other, and map an operator over the pairs
///
/// # Arguments
/// * `left` - left vector to map over
/// * `right` - right vector to map over
/// * `operator` - function to apply to each pair
///
/// # Return
/// An array of mapped data
///
/// # Example
/// ```
/// use whitenoise_validator::errors::*;
/// use ndarray::prelude::*;
/// use whitenoise_runtime::utilities::broadcast_map;
/// let left: ArrayD<f64> = arr1(&[1., -2., 3., 5.]).into_dyn();
/// let right: ArrayD<f64> = arr1(&[2.]).into_dyn();
/// let mapped: Result<ArrayD<f64>> = broadcast_map(&left, &right, &|l, r| l.max(r.clone()));
/// println!("{:?}", mapped); // [2., 2., 3., 5.]
/// ```
pub fn broadcast_map<T, U>(
    left: &ArrayD<T>,
    right: &ArrayD<T>,
    operator: &dyn Fn(&T, &T) -> U) -> Result<ArrayD<U>> where T: std::clone::Clone, U: Default {
    let shape = match left.ndim().cmp(&right.ndim()) {
        Ordering::Less => right.shape(),
        Ordering::Equal => if left.len() > right.len() { left.shape() } else { right.shape() },
        Ordering::Greater => left.shape()
    };

//    println!("shape {:?}", shape);
//    println!("left shape {:?}", left.shape());
//    println!("right shape {:?}", right.shape());
    // TODO: switch to array views to prevent the clone()
    let left = to_nd(left.clone(), &shape.len())?;
    let right = to_nd(right.clone(), &shape.len())?;

//    println!("shape {:?}", shape);
//    println!("left shape {:?}", left.shape());
//    println!("right shape {:?}", right.shape());
//    println!();

    let mut output: ArrayD<U> = ndarray::Array::default(shape);
    Zip::from(&mut output)
        .and(left.broadcast(shape).ok_or("could not broadcast left argument")?)
        .and(right.broadcast(shape).ok_or("could not broadcast right argument")?)
        .apply(|acc, l, r| *acc = operator(&l, &r));

    Ok(output)
}


#[cfg(test)]
mod test_broadcast_map {
    use ndarray::{arr0, arr1, arr2};
    use crate::utilities::broadcast_map;

    #[test]
    fn test_broadcasting() {
        let data0d = arr0(2.).into_dyn();
        let data1d = arr1(&[2., 3., 5.]).into_dyn();
        let data2d = arr2(&[[2., 4.], [3., 7.], [5., 2.]]).into_dyn();

        assert!(broadcast_map(
            &data0d, &data1d, &|l, r| l * r,
        ).unwrap() == arr1(&[4., 6., 10.]).into_dyn());

        assert!(broadcast_map(
            &data1d, &data2d, &|l, r| l / r,
        ).unwrap() == arr2(&[[1., 2./4.], [1., 3./7.], [1., 5. / 2.]]).into_dyn());

        assert!(broadcast_map(
            &data2d, &data0d, &|l, r| l + r,
        ).unwrap() == arr2(&[[4., 6.], [5., 9.], [7., 4.]]).into_dyn());
    }

    #[test]
    fn non_conformable() {
        let left = arr1(&[2., 3., 5.]).into_dyn();
        let right = arr1(&[2., 3., 5., 6.]).into_dyn();

        assert!(broadcast_map(
            &left, &right, &|l, r| l * r,
        ).is_err());
    }

    #[test]
    #[should_panic]
    fn arraynd_left_broadcast() {
        // if this test doesn't panic, then ndarray has added support for left broadcasting
        // once ndarray has support for left broadcasting, then evaluate removal of broadcast_map wherever possible
        let left = arr0(2.).into_dyn();
        let right = arr1(&[2., 3., 5., 6.]).into_dyn();

        let _broadcast = left / right;
    }
}

pub fn to_nd<T>(mut array: ArrayD<T>, ndim: &usize) -> Result<ArrayD<T>> {
    match (*ndim as i32) - (array.ndim() as i32) {
        0 => {}
        // must remove i axes
        i if i < 0 => {
            (0..-(i as i32)).map(|_| match array.shape().last()
                .ok_or_else(|| Error::from("ndim may not be negative"))? {
                1 => Ok(array.index_axis_inplace(Axis(array.ndim() - 1), 0)),
                _ => Err("cannot remove non-singleton trailing axis".into())
            }).collect::<Result<()>>()?
        }
        // must add i axes
        i if i > 0 => (0..i).for_each(|_| array.insert_axis_inplace(Axis(array.ndim()))),
        _ => return Err("invalid dimensionality".into())
    };

    Ok(array)
}


/// Return bytes of binary data as `String`.
///
/// Reads bytes from OpenSSL, converts them into a string,
/// concatenates them, and returns the combined string.
///
/// # Arguments
/// * `n_bytes` - The number of random bytes you wish to read from OpenSSL.
///
/// # Return
/// The `String` representation of the bytes.
pub fn get_bytes(n_bytes: usize) -> String {
    // read random bytes from OpenSSL
    let mut buffer = vec!(0_u8; n_bytes);
    rand_bytes(&mut buffer).unwrap();

    // create new buffer of binary representations, rather than u8
    let mut new_buffer = Vec::new();
    for i in 0..buffer.len() {
        new_buffer.push(format!("{:08b}", buffer[i]));
    }

    // combine binary representations into single string and subset mantissa
    let binary_string = new_buffer.join("");

    return binary_string;
}

/// Converts an `f64` to `String` of length 64, yielding the IEEE-754 binary representation of the `f64`.
///
/// The first bit of the string is the sign, the next 11 are the exponent, and the last 52 are the mantissa.
/// See [here](https://en.wikipedia.org/wiki/Double-precision_floating-point_format) for an explanation
/// of IEEE-754 64-bit floating-point numbers.
///
/// # Arguments
/// * `num` - A number of type f64.
///
/// # Return
/// A string showing the IEEE-754 binary representation of `num`.
pub fn f64_to_binary(num: &f64) -> String {
    // decompose num into component parts
    let (sign, exponent, mantissa) = num.decompose_raw();

    // convert each component into strings
    let sign_string = (sign as i64).to_string();
    let mantissa_string = format!("{:052b}", mantissa);
    let exponent_string = format!("{:011b}", exponent);

    // join component strings
    let binary_string = vec![sign_string, exponent_string, mantissa_string].join("");

    // return string representation
    return binary_string;
}

/// Converts `String` of length 64 to `f64`, yielding the floating-point number represented by the `String`.
///
/// The first bit of the string is the sign, the next 11 are the exponent, and the last 52 are the mantissa.
/// See [here](https://en.wikipedia.org/wiki/Double-precision_floating-point_format) for an explanation
/// of IEEE-754 64-bit floating-point numbers.
///
/// # Arguments
/// * `binary_string`: String showing IEEE-754 binary representation of a number
///
/// # Return
/// * `num`: f64 version of the String
pub fn binary_to_f64(binary_string: &String) -> f64 {
    // get sign and convert to bool as recompose expects
    let sign = &binary_string[0..1];
    let sign_bool = if sign.parse::<i32>().unwrap() == 0 {
        false
    } else {
        true
    };

    // convert exponent to int
    let exponent = &binary_string[1..12];
    let exponent_int = u16::from_str_radix(exponent, 2).unwrap();

    // convert mantissa to int
    let mantissa = &binary_string[12..];
    let mantissa_int = u64::from_str_radix(mantissa, 2).unwrap();

    // combine elements into f64 and return
    let num = f64::recompose_raw(sign_bool, exponent_int, mantissa_int);
    return num;
}

/// Takes `String` of form `{0,1}^64` and splits it into a sign, exponent, and mantissa
/// based on the IEEE-754 64-bit floating-point standard.
///
/// # Arguments
/// * `binary_string` - 64-bit binary string.
///
/// # Return
/// * `(sign, exponent, mantissa)` - where each is a `String`.
pub fn split_ieee_into_components(binary_string: &String) -> (String, String, String) {
    return (binary_string[0..1].to_string(), binary_string[1..12].to_string(), binary_string[12..].to_string());
}

/// Combines `String` versions of sign, exponent, and mantissa into
/// a single IEEE-754 64-bit floating-point representation.
///
/// # Arguments
/// * `sign` - Sign bit (length 1).
/// * `exponent` - Exponent bits (length 11).
/// * `mantissa` - Mantissa bits (length 52).
///
/// # Return
/// Concatenation of sign, exponent, and mantissa.
pub fn combine_components_into_ieee(sign: &str, exponent: &str, mantissa: &str) -> String {
    return vec![sign, exponent, mantissa].join("");
}

/// Samples a single element from a set according to provided weights.
///
/// # Arguments
/// * `candidate_set` - The set from which you want to sample.
/// * `weights` - Sampling weights for each element.
///
/// # Return
/// Element from the candidate set
pub fn sample_from_set<T>(candidate_set: &Vec<T>, weights: &Vec<f64>)
                          -> Result<T> where T: Clone {
    // generate uniform random number on [0,1)
    let unif: rug::Float = Float::with_val(53, noise::sample_uniform_mpfr(0., 1.)?);

    // generate sum of weights
    let weights_rug: Vec<rug::Float> = weights.into_iter().map(|w| Float::with_val(53, w)).collect();
    let weights_sum: rug::Float = Float::with_val(53, Float::sum(weights_rug.iter()));

    // NOTE: use this instead of the two lines above if we switch to accepting rug::Float rather than f64 weights
    // let weights_sum: rug::Float = Float::with_val(53, Float::sum(weights.iter()));

    // convert weights to probabilities
    let probabilities: Vec<rug::Float> = weights_rug.iter().map(|w| w / weights_sum.clone()).collect();

    // generate cumulative probability distribution
    let mut cumulative_probability_vec: Vec<rug::Float> = Vec::with_capacity(weights.len() as usize);
    for i in 0..weights.len() {
        cumulative_probability_vec.push(Float::with_val(53, Float::sum(probabilities[0..(i + 1)].iter())));
    }

    // sample an element relative to its probability
    let mut return_index = 0;
    for i in 0..cumulative_probability_vec.len() {
        if unif <= cumulative_probability_vec[i] {
            return_index = i;
            break;
        }
    }
    Ok(candidate_set[return_index as usize].clone())
}

///  Accepts an ndarray and returns the number of columns.
///
/// # Arguments
/// * `data` - The data for which you want to know the number of columns.
///
/// # Return
/// Number of columns in data.
pub fn get_num_columns<T>(data: &ArrayD<T>) -> Result<i64> {
    match data.ndim() {
        0 => Err("data is a scalar".into()),
        1 => Ok(1),
        2 => Ok(data.shape().last().unwrap().clone() as i64),
        _ => Err("data may be at most 2-dimensional".into())
    }
}
