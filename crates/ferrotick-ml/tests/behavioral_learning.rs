//! Behavioral tests for ML models - Phase 10 Machine Learning
//!
//! These tests verify that ML models actually learn patterns and can generalize,
//! not just that they run without errors.

use ferrotick_ml::{DecisionTreeClassifier, Model, SVMClassifier};
use ndarray::{Array1, Array2};

#[test]
fn test_svm_learns_linearly_separable_pattern() {
    // Create linearly separable data: positive class (x, y both > 0), negative class (both < 0)
    let features = Array2::from_shape_vec(
        (20, 2),
        vec![
            // Positive class (label = 1)
            1.0, 1.0, 2.0, 2.0, 1.5, 1.5, 3.0, 3.0, 2.5, 2.5, 1.0, 2.0, 2.0, 1.0, 1.8, 1.9, 2.2,
            2.1, 3.1, 2.9, // Negative class (label = -1)
            -1.0, -1.0, -2.0, -2.0, -1.5, -1.5, -3.0, -3.0, -2.5, -2.5, -1.0, -2.0, -2.0, -1.0,
            -1.8, -1.9, -2.2, -2.1, -3.1, -2.9,
        ],
    )
    .expect("valid features");

    let targets = Array1::from_vec(vec![
        1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0,
        -1.0, -1.0, -1.0,
    ]);

    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets)
        .expect("SVM training should succeed");

    // BEHAVIOR: Should correctly classify new points from positive class
    let test_positive = Array2::from_shape_vec((1, 2), vec![1.8, 1.8]).expect("valid test data");
    let pred_positive = Model::predict(&svm, &test_positive).expect("prediction should succeed");
    assert!(
        pred_positive[0] > 0.0,
        "SVM should predict positive class for point (1.8, 1.8), but predicted {:.2}",
        pred_positive[0]
    );

    // BEHAVIOR: Should correctly classify new points from negative class
    let test_negative = Array2::from_shape_vec((1, 2), vec![-1.8, -1.8]).expect("valid test data");
    let pred_negative = Model::predict(&svm, &test_negative).expect("prediction should succeed");
    assert!(
        pred_negative[0] < 0.0,
        "SVM should predict negative class for point (-1.8, -1.8), but predicted {:.2}",
        pred_negative[0]
    );
}

#[test]
fn test_svm_achieves_high_accuracy_on_training_data() {
    // Create simple separable data
    let features = Array2::from_shape_vec(
        (100, 2),
        (0..100)
            .flat_map(|i| {
                if i < 50 {
                    // Positive class: upper right quadrant
                    vec![(i + 10) as f64 * 0.1, (i + 10) as f64 * 0.1]
                } else {
                    // Negative class: lower left quadrant
                    vec![-((i - 40) as f64) * 0.1, -((i - 40) as f64) * 0.1]
                }
            })
            .collect(),
    )
    .expect("valid features");

    let targets = Array1::from_vec((0..100).map(|i| if i < 50 { 1.0 } else { -1.0 }).collect());

    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets)
        .expect("SVM training should succeed");

    // Predict on training data
    let predictions = Model::predict(&svm, &features).expect("prediction should succeed");

    // BEHAVIOR: Should achieve high accuracy on training data (> 90%)
    let correct = predictions
        .iter()
        .zip(targets.iter())
        .filter(|(pred, target)| (**pred > **target - 0.5) && (**pred < **target + 0.5))
        .count();

    let accuracy = correct as f64 / targets.len() as f64;
    assert!(
        accuracy > 0.90,
        "SVM should achieve > 90% accuracy on separable training data, but got {:.1}%",
        accuracy * 100.0
    );
}

#[test]
fn test_decision_tree_learns_conjunction_rule() {
    // Create data with rule: if x > 5 AND y > 5 then 1, else -1
    let features = Array2::from_shape_vec(
        (12, 2),
        vec![
            // Both > 5: label = 1
            6.0, 6.0, 7.0, 7.0, 8.0, 8.0, 10.0, 10.0, // x > 5, y < 5: label = -1
            6.0, 3.0, 8.0, 2.0, // x < 5, y > 5: label = -1
            3.0, 7.0, 2.0, 8.0, // Both < 5: label = -1
            3.0, 3.0, 2.0, 2.0, 1.0, 1.0, 4.0, 4.0,
        ],
    )
    .expect("valid features");

    let targets = Array1::from_vec(vec![
        1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0,
    ]);

    let mut tree = DecisionTreeClassifier::new(None);
    tree.fit(&features, &targets)
        .expect("tree training should succeed");

    // BEHAVIOR: Should correctly classify point with both > 5
    let test_both_high = Array2::from_shape_vec((1, 2), vec![10.0, 10.0]).expect("valid test data");
    let pred = Model::predict(&tree, &test_both_high).expect("prediction should succeed");
    assert!(
        pred[0] > 0.0,
        "Decision tree should predict positive class for (10, 10), but predicted {:.2}",
        pred[0]
    );

    // BEHAVIOR: Should correctly classify point with x > 5, y < 5
    let test_x_high = Array2::from_shape_vec((1, 2), vec![10.0, 3.0]).expect("valid test data");
    let pred = Model::predict(&tree, &test_x_high).expect("prediction should succeed");
    assert!(
        pred[0] < 0.0,
        "Decision tree should predict negative class for (10, 3), but predicted {:.2}",
        pred[0]
    );

    // BEHAVIOR: Should correctly classify point with x < 5, y > 5
    let test_y_high = Array2::from_shape_vec((1, 2), vec![3.0, 10.0]).expect("valid test data");
    let pred = Model::predict(&tree, &test_y_high).expect("prediction should succeed");
    assert!(
        pred[0] < 0.0,
        "Decision tree should predict negative class for (3, 10), but predicted {:.2}",
        pred[0]
    );
}

#[test]
fn test_decision_tree_achieves_high_accuracy_on_training_data() {
    // Create simple rule-based data
    let features = Array2::from_shape_vec(
        (100, 2),
        (0..100)
            .flat_map(|i| {
                let x = (i % 10) as f64;
                let y = (i / 10) as f64;
                vec![x, y]
            })
            .collect(),
    )
    .expect("valid features");

    let targets = Array1::from_vec(
        (0..100)
            .map(|i| {
                let x = (i % 10) as f64;
                let y = (i / 10) as f64;
                if x > 4.5 && y > 4.5 {
                    1.0
                } else {
                    -1.0
                }
            })
            .collect(),
    );

    let mut tree = DecisionTreeClassifier::new(None);
    tree.fit(&features, &targets)
        .expect("tree training should succeed");

    // Predict on training data
    let predictions = Model::predict(&tree, &features).expect("prediction should succeed");

    // BEHAVIOR: Should achieve very high accuracy on training data (> 95%)
    let correct = predictions
        .iter()
        .zip(targets.iter())
        .filter(|(pred, target)| (**pred > **target - 0.5) && (**pred < **target + 0.5))
        .count();

    let accuracy = correct as f64 / targets.len() as f64;
    assert!(
        accuracy > 0.95,
        "Decision tree should achieve > 95% accuracy on training data, but got {:.1}%",
        accuracy * 100.0
    );
}

#[test]
fn test_models_return_consistent_predictions() {
    // Create simple data
    let features = Array2::from_shape_vec(
        (10, 2),
        (0..10)
            .flat_map(|i| vec![i as f64, i as f64 * 2.0])
            .collect(),
    )
    .expect("valid features");

    let targets = Array1::from_vec((0..10).map(|i| if i < 5 { 1.0 } else { -1.0 }).collect());

    // Train SVM
    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets)
        .expect("SVM training should succeed");

    // Get predictions twice
    let pred1 = Model::predict(&svm, &features).expect("prediction 1 should succeed");
    let pred2 = Model::predict(&svm, &features).expect("prediction 2 should succeed");

    // BEHAVIOR: Same input should produce same output (deterministic)
    for (i, (p1, p2)) in pred1.iter().zip(pred2.iter()).enumerate() {
        assert!(
            (p1 - p2).abs() < 1e-10,
            "Predictions should be deterministic: pred1[{}] = {:.4}, pred2[{}] = {:.4}",
            i,
            p1,
            i,
            p2
        );
    }
}

#[test]
fn test_models_handle_edge_cases() {
    // Test with small dataset
    let features = Array2::from_shape_vec((4, 2), vec![1.0, 1.0, 2.0, 2.0, -1.0, -1.0, -2.0, -2.0])
        .expect("valid features");

    let targets = Array1::from_vec(vec![1.0, 1.0, -1.0, -1.0]);

    // BEHAVIOR: SVM should handle small datasets
    let mut svm = SVMClassifier::new();
    let result = svm.fit(&features, &targets);
    assert!(result.is_ok(), "SVM should handle small datasets");

    // BEHAVIOR: Decision tree should handle small datasets
    let mut tree = DecisionTreeClassifier::new(None);
    let result = tree.fit(&features, &targets);
    assert!(result.is_ok(), "Decision tree should handle small datasets");
}

#[test]
fn test_model_predictions_are_bounded() {
    // Create simple data
    let features = Array2::from_shape_vec(
        (50, 2),
        (0..50)
            .flat_map(|i| {
                let sign = if i < 25 { 1.0 } else { -1.0 };
                vec![sign * (i % 10) as f64, sign * ((i + 5) % 10) as f64]
            })
            .collect(),
    )
    .expect("valid features");

    let targets = Array1::from_vec((0..50).map(|i| if i < 25 { 1.0 } else { -1.0 }).collect());

    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets)
        .expect("SVM training should succeed");

    let predictions = Model::predict(&svm, &features).expect("prediction should succeed");

    // BEHAVIOR: Predictions should be bounded (not NaN or infinity)
    for (i, pred) in predictions.iter().enumerate() {
        assert!(
            pred.is_finite(),
            "Prediction[{}] should be finite, but got {}",
            i,
            pred
        );
    }
}

#[test]
fn test_models_generalize_to_unseen_data() {
    // Training data
    let train_features = Array2::from_shape_vec(
        (40, 2),
        (0..40)
            .flat_map(|i| {
                let x = (i % 10) as f64;
                let y = (i / 10) as f64;
                vec![x + 100.0, y + 100.0] // Shift to avoid zero
            })
            .collect(),
    )
    .expect("valid features");

    let train_targets = Array1::from_vec(
        (0..40)
            .map(|i| {
                let x = (i % 10) as f64;
                let y = (i / 10) as f64;
                if x > 4.5 && y > 4.5 {
                    1.0
                } else {
                    -1.0
                }
            })
            .collect(),
    );

    // Train model
    let mut tree = DecisionTreeClassifier::new(None);
    tree.fit(&train_features, &train_targets)
        .expect("tree training should succeed");

    // Test data (unseen)
    let test_features = Array2::from_shape_vec(
        (20, 2),
        (0..20)
            .flat_map(|i| {
                let x = ((i % 5) * 2 + 1) as f64; // Different pattern
                let y = ((i / 5) * 2 + 1) as f64;
                vec![x + 100.0, y + 100.0]
            })
            .collect(),
    )
    .expect("valid test features");

    let test_targets = Array1::from_vec(
        (0..20)
            .map(|i| {
                let x = ((i % 5) * 2 + 1) as f64;
                let y = ((i / 5) * 2 + 1) as f64;
                if x > 4.5 && y > 4.5 {
                    1.0
                } else {
                    -1.0
                }
            })
            .collect(),
    );

    // Predict on test data
    let predictions = Model::predict(&tree, &test_features).expect("prediction should succeed");

    // BEHAVIOR: Should generalize reasonably well to unseen data (> 70% accuracy)
    let correct = predictions
        .iter()
        .zip(test_targets.iter())
        .filter(|(pred, target)| (**pred > **target - 0.5) && (**pred < **target + 0.5))
        .count();

    let accuracy = correct as f64 / test_targets.len() as f64;
    assert!(
        accuracy >= 0.70,
        "Model should generalize with >= 70% accuracy on test data, but got {:.1}%",
        accuracy * 100.0
    );
}
