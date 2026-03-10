pub mod decision_tree;
pub mod svm;
pub mod traits;

pub use decision_tree::DecisionTreeClassifier;
pub use svm::SVMClassifier;
pub use traits::{Model, PersistentModel};
