use ferrotick_ml::{SVMClassifier, Model};
use ndarray::{Array1, Array2};

#[test]
fn test_svm_basic() {
    // Create simple synthetic data
    let features = Array2::from_shape_vec(
        (100, 2),
        (0..100)
            .flat_map(|i| {
                let x = if i < 50 { i as f64 } else { i as f64 + 50.0 };
                vec![x, x * 0.5]
            })
            .collect::<Vec<f64>>(),
    )
    .unwrap();

    let targets = Array1::from_vec((0..100).map(|i| if i < 50 { -1.0 } else { 1.0 }).collect());

    // Train SVM
    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets).expect("training failed");

    // Predict using Model trait (batch prediction)
    let predictions = Model::predict(&svm, &features).expect("prediction failed");
    assert_eq!(predictions.len(), 100);
    
    // Check that predictions are in expected range (-1 or 1)
    for pred in predictions.iter() {
        assert!(*pred == -1.0 || *pred == 1.0);
    }
}
