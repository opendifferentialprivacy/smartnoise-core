//! Component trait implementations
//!
//! Each component represents an abstract computation.
//! Example components are Materialize for loading a dataframe, Index for retrieving specific columns from the dataframe, Mean for aggregating data, LaplaceMechanism for privatizing data, etc.
//!
//! There are a set of possible behaviours each component may implement. Each behavior corresponds to a trait. A listing of traits is at the bottom of the page.
//!
//! Implementations of the traits are distributed among the module files.

use crate::errors::*;


mod transforms;
//mod bin;
mod cast;
mod clamp;
mod count;
mod covariance;
mod column_bind;
mod digitize;
mod dp_count;
mod dp_variance;
mod dp_covariance;
mod dp_histogram;
mod dp_maximum;
mod dp_median;
mod dp_minimum;
mod dp_mean;
mod dp_quantile;
mod dp_raw_moment;
mod dp_sum;
mod filter;
mod histogram;
mod impute;
pub mod index;
mod raw_moment;
mod literal;
mod map;
mod materialize;
pub mod partition;
mod quantile;
mod reshape;
mod mean;
mod exponential_mechanism;
mod gaussian_mechanism;
mod laplace_mechanism;
mod simple_geometric_mechanism;
mod resize;
mod sum;
mod union;
mod variance;

use crate::base::{IndexKey, Value, NodeProperties, SensitivitySpace, ValueProperties};
use crate::{proto, Warnable, base};
use crate::utilities::json::{JSONRelease};
use indexmap::map::IndexMap;

/// Universal Component trait
///
/// To be a component, a struct must represent an abstract computation, for which properties can be derived about the resulting data.
pub trait Component {
    /// Given properties known about private arguments, and public arguments, derive properties about the resulting data.
    ///
    /// A component must fail to propagate properties if requirements on the input properties are not met.
    /// For example, if a Component represents an abstract computation that requires prior knowledge of the number of records to be safe or function properly,
    /// the propagate_property implementation is expected to return an error state if the prior knowledge is not known.
    ///
    /// For example, if a definition of privacy is used that is incompatible with the abstract computation,
    /// the propagate_property implementation is expected to return an error state.
    ///
    /// # Arguments
    /// * `self` - the protobuf object corresponding to the prost protobuf struct
    /// * `privacy_definition` - the definition of privacy under which the computation takes place
    /// * `public_arguments` - actual data values of arguments, typically either supplied literals or released values.
    /// * `properties` - derived properties of private input arguments
    /// * `node_id` - id of the node in the analysis graph (used to set dataset_id in the data loaders)
    ///
    /// # Returns
    /// Derived properties on the data resulting from the abstract computation
    fn propagate_property(
        &self,
        privacy_definition: &Option<proto::PrivacyDefinition>,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: NodeProperties,
        _node_id: u32,
    ) -> Result<Warnable<ValueProperties>>;
}

/// Expandable Component trait
///
/// When a component is expandable, it represents a higher order computation that may be expressed in multiple components that are more granular.
/// Oftentimes Expandable components correspond to differentially private algorithms,
/// that are represented in terms of an aggregation and a mechanism.
pub trait Expandable {
    /// Concrete implementation for an Expandable component that returns a patch that may be applied to a computation graph.
    ///
    /// # Arguments
    /// * `self` - the protobuf object corresponding to the prost protobuf struct
    /// * `privacy_definition` - definition of privacy to use when expanding. Some expansions are not valid under some privacy definitions
    /// * `component` - contains additional metadata about the argument node ids
    /// * `properties` - properties on the data supplied as arguments
    /// * `component_id` - the id of the node to expand. The final node in the returned patch must use this id.
    /// * `maximum_id` - the starting id for which additional nodes may be added to the graph without overwriting existing nodes
    ///
    /// # Returns
    /// Sufficient information to patch the runtime with more granular steps.
    /// More documentation at [ComponentExpansion](proto::ComponentExpansion).
    fn expand_component(
        &self,
        privacy_definition: &Option<proto::PrivacyDefinition>,
        component: &proto::Component,
        public_arguments: &IndexMap<base::IndexKey, &Value>,
        properties: &NodeProperties,
        component_id: u32,
        maximum_id: u32,
    ) -> Result<base::ComponentExpansion>;
}

/// Mechanism component trait
///
/// When a component is a Mechanism, it consumes a privacy budget.
pub trait Mechanism {
    /// Extraction of privacy usage by the component.
    ///
    /// By default, this returns the upper bound of the privacy usage of the component.
    ///
    /// If the component has been evaluated, it is possible the actual usage of the component differs from the upper bound.
    /// In this case, the release_usage is returned.
    ///
    /// # Arguments
    /// * `self` - the protobuf object corresponding to the prost protobuf struct, containing an upper bound on privacy usage
    /// * `privacy_definition` - the definition of privacy under which the sensitivity is to be computed
    /// * `release_usage` - optionally, the privacy actually used by the mechanism (if it has already been released)
    /// * `sensitivity_type` - space for which the sensitivity is computed within
    ///
    /// # Returns
    /// Privacy usages after group_size, c_stability and privacy amplification have been taken into account.
    fn get_privacy_usage(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        release_usage: Option<&Vec<proto::PrivacyUsage>>,
        properties: &NodeProperties
    ) -> Result<Option<Vec<proto::PrivacyUsage>>>;
}

/// Sensitivity component trait
///
/// When a component has sensitivity, the abstract computation the component represents combines multiple rows together into a single value.
/// For example, a mean, minimum, or scoring function on a dataset. A component that aggregates data has an associated sensitivity, which captures
/// how much the input data affects the output of the aggregator.
pub trait Sensitivity {
    /// Derivation for the sensitivity of an aggregator based on available local metadata.
    ///
    /// The sensitivity is the maximum amount that a perturbation of input data may have on the resulting value.
    /// The type of perturbation is described in the privacy_definition.
    ///
    /// # Arguments
    /// * `self` - the protobuf object corresponding to the prost protobuf struct
    /// * `privacy_definition` - the definition of privacy under which the sensitivity is to be computed
    /// * `properties` - derived properties for the input data
    /// * `sensitivity_type` - space for which the sensitivity is computed within
    ///
    /// # Returns
    /// Sensitivities for each of the values in the resulting computation
    fn compute_sensitivity(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        sensitivity_type: &SensitivitySpace,
    ) -> Result<Value>;
}

/// Accuracy component trait
///
/// Components with Accuracy implemented may convert between privacy units and accuracy estimates
pub trait Accuracy {
    fn accuracy_to_privacy_usage(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        accuracies: &proto::Accuracies,
    ) -> Result<Option<Vec<proto::PrivacyUsage>>>;

    fn privacy_usage_to_accuracy(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        alpha: f64,
    ) -> Result<Option<Vec<proto::Accuracy>>>;
}

/// Report component trait
///
/// Reportable components correspond to a computation that a researcher may want a JSON summary for
pub trait Report {
    /// Summarize the relevant metadata around a computation in a readable, JSON-serializable format.
    fn summarize(
        &self,
        node_id: u32,
        component: &proto::Component,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: NodeProperties,
        release: &Value,
        variable_names: Option<&Vec<base::IndexKey>>,
    ) -> Result<Option<Vec<JSONRelease>>>;
}

/// Named component trait
///
/// Named components involve variables and keep track of the human readable names for these variables
/// and may modify these variables names.
pub trait Named {
    /// Propagate the human readable names of the variables associated with this component
    fn get_names(
        &self,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        argument_variables: IndexMap<base::IndexKey, Vec<IndexKey>>,
        release: Option<&Value>,
    ) -> Result<Vec<IndexKey>>;
}


impl Component for proto::Component {
    /// Utility implementation on the component.
    ///
    /// This utility delegates evaluation to the concrete implementation of each component variant.
    fn propagate_property(
        &self,
        privacy_definition: &Option<proto::PrivacyDefinition>,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: NodeProperties,
        node_id: u32,
    ) -> Result<Warnable<ValueProperties>> {
        let variant = self.variant.as_ref()
            .ok_or_else(|| "variant: must be defined")?;

        macro_rules! propagate_property {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = variant {
                            return x.propagate_property(privacy_definition, public_arguments, properties, node_id)
                                .chain_err(|| format!("node specification {:?}:", variant))
                       }
                    )*
                }
            }
        }

        propagate_property!(
            // INSERT COMPONENT LIST
            Cast, Clamp, ColumnBind, Count, Covariance, Digitize,
            Filter, Histogram, Impute, Index, Literal, Materialize, Mean,
            Partition, Quantile, RawMoment, Reshape, Resize, Sum, Union, Variance,

            ExponentialMechanism, GaussianMechanism, LaplaceMechanism, SimpleGeometricMechanism,

            Abs, Add, LogicalAnd, Divide, Equal, GreaterThan, LessThan, Log, Modulo, Multiply,
            Negate, Negative, LogicalOr, Power, RowMax, RowMin, Subtract
        );

        Err(format!("proto component {:?} is missing its Component trait", variant).into())
    }
}

impl Expandable for proto::Component {
    /// Utility implementation on the component.
    ///
    /// This utility delegates evaluation to the concrete implementation of each component variant.
    fn expand_component(
        &self,
        privacy_definition: &Option<proto::PrivacyDefinition>,
        component: &proto::Component,
        public_arguments: &IndexMap<base::IndexKey, &Value>,
        properties: &NodeProperties,
        component_id: u32,
        maximum_id: u32,
    ) -> Result<base::ComponentExpansion> {
        let variant = self.variant.as_ref()
            .ok_or_else(|| "variant: must be defined")?;

        macro_rules! expand_component {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = variant {
                            let expansion = x.expand_component(
                                privacy_definition, component, public_arguments,
                                properties, component_id, maximum_id)
                                .chain_err(|| format!("node specification {:?}:", variant))?;

                            expansion.is_valid(component_id)?;
                            return Ok(expansion)
                       }
                    )*
                }
            }
        }

        // indexes and unions accept partitioned data as an argument- don't expand with map
        if let proto::component::Variant::Index(_) = variant {
            return Ok(base::ComponentExpansion::default())
        }
        if let proto::component::Variant::Union(_) = variant {
            return Ok(base::ComponentExpansion::default())
        }

        // list all components that accept partitioned data as arguments
        expand_component!(Map);

        if properties.values().any(|props| props.partitions().is_ok()) {
            let mut component_expansion = base::ComponentExpansion::default();
            component_expansion.computation_graph.insert(component_id, proto::Component {
                arguments: component.arguments.clone(),
                variant: Some(proto::component::Variant::Map(Box::new(proto::Map {
                    component: Some(Box::from(component.clone()))
                }))),
                omit: component.omit,
                submission: component.submission,
            });
            component_expansion.traversal.push(component_id);
            return Ok(component_expansion);
        }

        expand_component!(
            // INSERT COMPONENT LIST
            Clamp, Digitize, Histogram, Impute, Map, Maximum, Median, Minimum, Partition, Resize,

            DpCount, DpCovariance, DpHistogram, DpMaximum, DpMean, DpMedian,
            DpMinimum, DpQuantile, DpRawMoment, DpSum, DpVariance,

            ExponentialMechanism, GaussianMechanism, LaplaceMechanism, SimpleGeometricMechanism,

            ToBool, ToFloat, ToInt, ToString
        );

        // no expansion
        Ok(base::ComponentExpansion::default())
    }
}

impl Mechanism for proto::Component {

    fn get_privacy_usage(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        release_usage: Option<&Vec<proto::PrivacyUsage>>,
        properties: &NodeProperties
    ) -> Result<Option<Vec<proto::PrivacyUsage>>> {
        let variant = self.variant.as_ref()
            .ok_or_else(|| "variant: must be defined")?;

        macro_rules! get_privacy_usage {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = variant {
                            return x.get_privacy_usage(privacy_definition, release_usage, properties)
                                .chain_err(|| format!("node specification {:?}:", variant))
                       }
                    )*
                }
            }
        }

        get_privacy_usage!(
            // INSERT COMPONENT LIST
            ExponentialMechanism, GaussianMechanism, LaplaceMechanism, SimpleGeometricMechanism
        );

        Ok(None)
    }
}


impl Sensitivity for proto::component::Variant {
    /// Utility implementation on the enum containing all variants of a component.
    ///
    /// This utility delegates evaluation to the concrete implementation of each component variant.
    fn compute_sensitivity(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        sensitivity_type: &SensitivitySpace,
    ) -> Result<Value> {
        macro_rules! compute_sensitivity {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = self {
                            return x.compute_sensitivity(privacy_definition, properties, sensitivity_type)
                                .chain_err(|| format!("node specification {:?}:", self))
                       }
                    )*
                }
            }
        }

        compute_sensitivity!(
            // INSERT COMPONENT LIST
            Count, Covariance, Histogram, Mean, Quantile, RawMoment, Sum, Union, Variance
        );

        Err(format!("sensitivity is not implemented for proto component {:?}", self).into())
    }
}

impl Accuracy for proto::Component {
    /// Utility implementation on the component.
    ///
    /// This utility delegates evaluation to the concrete implementation of each component variant.
    fn accuracy_to_privacy_usage(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        accuracy: &proto::Accuracies,
    ) -> Result<Option<Vec<proto::PrivacyUsage>>> {
        let variant = self.variant.as_ref()
            .ok_or_else(|| "variant: must be defined")?;

        macro_rules! accuracy_to_privacy_usage {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = variant {
                            return x.accuracy_to_privacy_usage(privacy_definition, properties, accuracy)
                                .chain_err(|| format!("node specification {:?}:", variant))
                       }
                    )*
                }
            }
        }

        accuracy_to_privacy_usage!(
             LaplaceMechanism,
             GaussianMechanism,
             SimpleGeometricMechanism
        );

        Ok(None)
    }

    /// Utility implementation on the component.
    ///
    /// This utility delegates evaluation to the concrete implementation of each component variant.
    fn privacy_usage_to_accuracy(
        &self,
        privacy_definition: &proto::PrivacyDefinition,
        properties: &NodeProperties,
        alpha: f64,
    ) -> Result<Option<Vec<proto::Accuracy>>> {
        let variant = self.variant.as_ref()
            .ok_or_else(|| "variant: must be defined")?;

        macro_rules! privacy_usage_to_accuracy {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = variant {
                            return x.privacy_usage_to_accuracy(privacy_definition, properties, alpha)
                                .chain_err(|| format!("node specification {:?}:", variant))
                       }
                    )*
                }
            }
        }

        privacy_usage_to_accuracy!(
            LaplaceMechanism,
            GaussianMechanism,
            SimpleGeometricMechanism
        );

        Ok(None)
    }
}

impl Report for proto::Component {
    /// Utility implementation on the component.
    ///
    /// This utility delegates evaluation to the concrete implementation of each component variant.
    fn summarize(
        &self,
        node_id: u32,
        component: &proto::Component,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        properties: NodeProperties,
        release: &Value,
        variable_names: Option<&Vec<base::IndexKey>>,
    ) -> Result<Option<Vec<JSONRelease>>> {
        let variant = self.variant.as_ref()
            .ok_or_else(|| "variant: must be defined")?;

        macro_rules! summarize {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = variant {
                            return x.summarize(node_id, component, public_arguments,
                                 properties, release, variable_names)
                                .chain_err(|| format!("node specification: {:?}:", variant))
                       }
                    )*
                }
            }
        }

        summarize!(
            // INSERT COMPONENT LIST
            DpCount, DpCovariance, DpHistogram, DpMaximum, DpMean, DpMinimum, DpQuantile,
            DpRawMoment, DpSum, DpVariance
        );

        Ok(None)
    }
}

impl Named for proto::Component {
    /// Utility implementation on the component.
    ///
    /// This utility delegates evaluation to the concrete implementation of each component variant.
    fn get_names(
        &self,
        public_arguments: IndexMap<base::IndexKey, &Value>,
        argument_variables: IndexMap<base::IndexKey, Vec<IndexKey>>,
        release: Option<&Value>,
    ) -> Result<Vec<IndexKey>> {
        let variant = self.variant.as_ref()
            .ok_or_else(|| "variant: must be defined")?;

        macro_rules! get_names {
            ($( $variant:ident ),*) => {
                {
                    $(
                       if let proto::component::Variant::$variant(x) = variant {
                            return x.get_names(public_arguments, argument_variables, release)
                                .chain_err(|| format!("node specification {:?}:", variant))
                       }
                    )*
                }
            }
        }

        // TODO: transforms, covariance/cross-covariance, extended indexing
        get_names!(
            // INSERT COMPONENT LIST
            ColumnBind, Index, Literal, Materialize
        );

        // default implementation
        match argument_variables.get(&IndexKey::from("data")) {
            // by convention, names pass through the "data" argument unchanged
            Some(variable_names) => Ok(variable_names.clone()),
            // otherwise if the component is non-standard, throw an error
            None => Err(format!("names are not implemented for proto component {:?}", variant).into())
        }
    }
}

