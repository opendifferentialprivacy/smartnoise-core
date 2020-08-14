//! Work-in-progress shorthand interface for building differentially private analyses.
//! Bundled with the rust validator.
//!
//! The Analysis struct has impl's for each component variant, that returns a builder object.
//! Mandatory arguments are supplied in analysis impl, but optional arguments and evaluated values may be set via the builder.
//! Once the component is ready to add to the analysis, call enter() on the builder to get a node id of the component.
//!
//! # Example
//! ```
//! use whitenoise_validator::bindings::Analysis;
//! use ndarray::arr1;
//! let mut analysis = Analysis::new();
//! let lit_2 = analysis.literal().value(2.0.into()).build();
//! let lit_3 = analysis.literal().value(3.0.into()).build();
//! let _lit_5 = analysis.add(lit_2, lit_3).build();
//!
//! let col_a = analysis.literal()
//!     .value(arr1(&[1., 2., 3.]).into_dyn().into())
//!     .build();
//! analysis.mean(col_a).build();
//!
//! analysis.count(col_a).build();
//! println!("graph {:?}", analysis.components);
//! println!("release {:?}", analysis.release);
//! ```

use crate::{proto, get_properties};
use crate::base::{Release, ValueProperties};
use std::collections::HashMap;
use crate::errors::*;


#[derive(Debug, Default)]
pub struct Analysis {
    pub privacy_definition: proto::PrivacyDefinition,
    pub components: HashMap<u32, proto::Component>,
    pub component_count: u32,
    pub submission_count: u32,
    pub release: Release,
}

impl Analysis {
    pub fn new() -> Self {
        Analysis {
            privacy_definition: proto::PrivacyDefinition {
                group_size: 1,
                neighboring: proto::privacy_definition::Neighboring::AddRemove as i32,
                strict_parameter_checks: false,
                protect_overflow: false,
                protect_elapsed_time: false,
                protect_memory_utilization: false,
                protect_floating_point: false
            },
            components: HashMap::new(),
            component_count: 0,
            submission_count: 0,
            release: Release::new(),
        }
    }

    pub fn properties(&self, id: u32) -> Result<ValueProperties> {
        let (properties, warnings) = get_properties(
            Some(self.privacy_definition.clone()),
            self.components.clone(),
            self.release.clone(),
            vec![id]
        )?;

        if !warnings.is_empty() {
            bail!("{:?}", warnings)
        }

        properties.get(&id).cloned()
            .ok_or_else(|| Error::from(format!("Failure to propagate properties to node {}", id)))
    }
}

include!(concat!(env!("OUT_DIR"), "/bindings_analysis.rs"));

pub mod builders {
    include!(concat!(env!("OUT_DIR"), "/bindings_builders.rs"));
}

#[cfg(test)]
mod test_bindings {
    use crate::bindings::Analysis;
    use crate::bindings::*;
    use ndarray::arr1;

    fn build_analysis() -> Result<()> {
        let mut analysis = Analysis::new();

        let lit_2 = analysis.literal().value(2.0.into()).build();
        let lit_3 = analysis.literal().value(3.0.into()).build();
        let _lit_5 = analysis.add(lit_2, lit_3).build();

        let col_a = analysis.literal()
            .value(arr1(&[1., 2., 3.]).into_dyn().into())
            .build();
        analysis.mean(col_a).build();

        analysis.count(col_a).build();
        // println!("graph {:?}", analysis.components);
        // println!("release {:?}", analysis.release);
        Ok(())
    }

    #[test]
    fn test_analysis() {
        build_analysis().unwrap();
    }
}


