use ferrotick_ml::{Model, SVMClassifier};
use ndarray::{Array1, Array2};
use tempfile::tempdir;

#[test]
fn test_svm_basic() {
    // Create linearly separable training data
    let features = Array2::from_shape_vec(
        (12, 2),
        vec![
            -4.0, -3.5, -3.5, -2.0, -3.0, -4.0, -2.5, -1.5, -2.0, -3.0, -1.5, -2.5, 2.0, 1.0, 2.5,
            1.5, 3.0, 2.0, 3.5, 2.5, 4.0, 3.0, 2.0, 2.5,
        ],
    )
    .unwrap();
    let targets = Array1::from_vec(vec![
        -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ]);

    // Holdout test set in the same separable regime
    let test_features = Array2::from_shape_vec(
        (10, 2),
        vec![
            -5.0, -4.0, -3.2, -2.7, -2.1, -2.2, -1.2, -1.5, -2.8, -3.1, 1.2, 1.1, 2.8, 2.2, 3.2,
            2.9, 4.2, 3.7, 1.8, 1.9,
        ],
    )
    .unwrap();
    let test_targets =
        Array1::from_vec(vec![-1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);

    // Train SVM
    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets).expect("training failed");

    // Predict using Model trait (batch prediction) and verify held-out accuracy
    let predictions = Model::predict(&svm, &test_features).expect("prediction failed");
    assert_eq!(predictions.len(), test_targets.len());

    let correct = predictions
        .iter()
        .zip(test_targets.iter())
        .filter(|(pred, expected)| (**pred - **expected).abs() < f64::EPSILON)
        .count();
    let accuracy = correct as f64 / test_targets.len() as f64;
    assert!(
        accuracy >= 0.9,
        "expected >=90% accuracy, got {:.2}%",
        accuracy * 100.0
    );

    // Check that predictions are in expected range (-1 or 1) and match expected labels
    for pred in predictions.iter() {
        assert!(*pred == -1.0 || *pred == 1.0);
    }
    assert_eq!(predictions[0], -1.0);
    assert_eq!(predictions[5], 1.0);
    assert_eq!(predictions[9], 1.0);
}

#[test]
fn test_svm_save_load_round_trip() {
    let features = Array2::from_shape_vec(
        (12, 2),
        vec![
            -4.0, -3.5, -3.5, -2.0, -3.0, -4.0, -2.5, -1.5, -2.0, -3.0, -1.5, -2.5, 2.0, 1.0, 2.5,
            1.5, 3.0, 2.0, 3.5, 2.5, 4.0, 3.0, 2.0, 2.5,
        ],
    )
    .unwrap();
    let targets = Array1::from_vec(vec![
        -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ]);

    let test_features = Array2::from_shape_vec(
        (6, 2),
        vec![
            -5.0, -4.0, -3.2, -2.7, -2.1, -2.2, 1.2, 1.1, 2.8, 2.2, 3.2, 2.9,
        ],
    )
    .unwrap();

    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets).expect("training failed");
    let baseline = Model::predict(&svm, &test_features).expect("prediction failed");

    let dir = tempdir().expect("tempdir should be created");
    let path = dir.path().join("svm.json");
    svm.save(&path).expect("save should succeed");

    let loaded = SVMClassifier::load(&path).expect("load should succeed");
    let reloaded = Model::predict(&loaded, &test_features).expect("prediction failed");

    assert_eq!(baseline, reloaded);
}
