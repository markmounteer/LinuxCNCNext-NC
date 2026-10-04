use nextnc_task_ffi::nextnc_task_spindle_identity;

#[test]
fn bounded_host_witness_hashes_deterministically_and_refuses_bad_extents() {
    let witness = b"host-observed test graph, not execution permission";
    let mut first = [0; 32];
    assert_eq!(
        // SAFETY: disjoint valid input and output with exact extents.
        unsafe {
            nextnc_task_spindle_identity(
                witness.as_ptr(),
                witness.len() as u64,
                first.as_mut_ptr(),
                first.len() as u64,
            )
        },
        0
    );
    assert_ne!(first, [0; 32]);
    assert_eq!(
        first,
        nextnc_task::spindle::witness_identity(witness).unwrap_or([0; 32])
    );
    let mut second = [0; 32];
    assert_eq!(
        // SAFETY: as above; shortened input remains valid and changes the identity.
        unsafe {
            nextnc_task_spindle_identity(
                witness.as_ptr(),
                witness.len() as u64 - 1,
                second.as_mut_ptr(),
                32,
            )
        },
        0
    );
    assert_ne!(first, second);
    for length in [0, 16_385, u64::MAX] {
        second.fill(42);
        assert_eq!(
            // SAFETY: invalid length is rejected before reading the valid pointer.
            unsafe {
                nextnc_task_spindle_identity(witness.as_ptr(), length, second.as_mut_ptr(), 32)
            },
            -1
        );
        assert_eq!(second, [0; 32]);
    }
    second.fill(42);
    assert_eq!(
        // SAFETY: short output is refused before writes; memory remains valid.
        unsafe {
            nextnc_task_spindle_identity(
                witness.as_ptr(),
                witness.len() as u64,
                second.as_mut_ptr(),
                31,
            )
        },
        -1
    );
    assert_eq!(second, [42; 32]);
    assert_eq!(
        // SAFETY: null input is refused before reading; output has full capacity.
        unsafe { nextnc_task_spindle_identity(std::ptr::null(), 1, second.as_mut_ptr(), 32) },
        -1
    );
    assert_eq!(second, [0; 32]);
    assert!(nextnc_task::spindle::witness_identity(&[]).is_err());
    assert!(nextnc_task::spindle::witness_identity(&[0; 16_385]).is_err());
    assert!(nextnc_task::spindle::witness_identity(&[0; 16_384]).is_ok());
}
