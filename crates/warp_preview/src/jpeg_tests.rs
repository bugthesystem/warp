use super::*;

#[test]
fn rgb_round_trips_through_jpeg_with_its_size() {
    let rgb = vec![200u8; 8 * 4 * 3];
    let frame = frame_from_rgb(&rgb, 8, 4).expect("encodes");
    assert_eq!((frame.width, frame.height), (8, 4));

    let decoded = frame_from_jpeg(frame.jpeg.to_vec()).expect("decodes");
    assert_eq!((decoded.width, decoded.height), (8, 4));
}

#[test]
fn bytes_that_are_not_jpeg_give_no_frame() {
    assert_eq!(frame_from_jpeg(b"not a jpeg".to_vec()), None);
}

#[test]
fn shrink_limits_the_longest_side() {
    let frame = frame_from_rgb(&vec![90u8; 40 * 20 * 3], 40, 20).expect("encodes");
    let small = shrink(&frame.jpeg, 10).expect("shrinks");
    let decoded = frame_from_jpeg(small).expect("decodes");
    assert_eq!((decoded.width, decoded.height), (10, 5));

    let same = shrink(&frame.jpeg, 100).expect("keeps");
    assert_eq!(same, frame.jpeg.to_vec());
}
