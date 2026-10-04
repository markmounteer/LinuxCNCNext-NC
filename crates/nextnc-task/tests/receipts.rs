use nextnc_task::{
    lifecycle::{Binding, Error, Generation},
    receipts::*,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn binding() -> Binding {
    Binding {
        generation: Generation {
            owner: 1,
            selection: 1,
        },
        state_epoch: 1,
        state_sha256: [4; 32],
        artifact_sha256: [5; 32],
    }
}

#[test]
fn a_command_is_admitted_only_after_all_of_its_task_pieces_are_accepted() -> TestResult {
    let b = binding();
    let mut ledger = Ledger::new(b, 4)?;
    assert_eq!(
        ledger.enqueue(b, 0, &[Recipient::Task, Recipient::Motion])?,
        1..3
    );
    assert_eq!(ledger.enqueue(b, 1, &[Recipient::Io])?, 3..4);
    let first = ledger.begin_issue(1)?.ok_or("first piece")?;
    assert_eq!(ledger.begin_issue(1), Err(Error::Busy));
    assert_eq!(ledger.acknowledge(first, Outcome::Accepted, 1)?.admitted, 0);
    let second = ledger.begin_issue(1)?.ok_or("second piece")?;
    assert_eq!(second.command, first.command);
    assert_eq!(second.piece, 1);
    assert_eq!(second.recipient, Recipient::Motion);
    assert_eq!(
        ledger.acknowledge(second, Outcome::Accepted, 2)?.admitted,
        1
    );
    assert!(!ledger.observed_after_dispatch(3));
    let third = ledger.begin_issue(2)?.ok_or("third piece")?;
    assert_eq!(ledger.acknowledge(third, Outcome::Accepted, 2)?.admitted, 2);
    assert_eq!(
        ledger.counts(),
        Counts {
            queued_commands: 2,
            admitted_commands: 2,
            queued_pieces: 3,
            accepted_pieces: 3,
            pending_pieces: 0
        }
    );
    assert!(!ledger.observed_after_dispatch(2));
    assert!(ledger.observed_after_dispatch(3));
    Ok(())
}

#[test]
fn receipt_retry_never_reissues_motion_or_counts_an_ack_twice() -> TestResult {
    let b = binding();
    let mut ledger = Ledger::new(b, 2)?;
    ledger.enqueue(b, 0, &[Recipient::Motion])?;
    let ticket = ledger.begin_issue(10)?.ok_or("motion")?;
    let receipt = ledger.acknowledge(ticket, Outcome::Accepted, 11)?;
    let counts = ledger.counts();
    assert_eq!(ledger.acknowledge(ticket, Outcome::Accepted, 11)?, receipt);
    assert_eq!(
        ledger.acknowledge(ticket, Outcome::Rejected, 11),
        Err(Error::Sequence)
    );
    assert_eq!(
        ledger.acknowledge(ticket, Outcome::Accepted, 12),
        Err(Error::Sequence)
    );
    assert_eq!(ledger.counts(), counts);
    assert_eq!(ledger.begin_issue(12)?, None);
    ledger.enqueue(b, 1, &[Recipient::Motion])?;
    let next = ledger.begin_issue(12)?.ok_or("second motion")?;
    assert_eq!(ledger.acknowledge(ticket, Outcome::Accepted, 11)?, receipt);
    assert_eq!(ledger.begin_issue(12), Err(Error::Busy));
    ledger.acknowledge(next, Outcome::Accepted, 12)?;
    assert_eq!(ledger.counts().admitted_commands, 2);
    assert_eq!(
        ledger.acknowledge(ticket, Outcome::Accepted, 11),
        Err(Error::Sequence)
    );
    Ok(())
}

#[test]
fn rejected_or_uncertain_delivery_closes_before_any_suffix_can_be_issued() -> TestResult {
    for outcome in [Outcome::Rejected, Outcome::Unknown] {
        let b = binding();
        let mut ledger = Ledger::new(b, 4)?;
        ledger.enqueue(b, 0, &[Recipient::Task, Recipient::Motion])?;
        ledger.enqueue(b, 1, &[Recipient::Motion])?;
        let first = ledger.begin_issue(1)?.ok_or("state")?;
        ledger.acknowledge(first, Outcome::Accepted, 1)?;
        let failed = ledger.begin_issue(2)?.ok_or("motion")?;
        let receipt = ledger.acknowledge(failed, outcome, 3)?;
        assert!(ledger.closed());
        assert_eq!(receipt.admitted, 0);
        assert_eq!(ledger.counts().accepted_pieces, 1);
        assert_eq!(ledger.last_receipt(), Some(receipt));
        assert_eq!(ledger.acknowledge(failed, outcome, 3)?, receipt);
        assert_eq!(ledger.begin_issue(4), Err(Error::State));
        assert_eq!(
            ledger.enqueue(b, 2, &[Recipient::Motion]),
            Err(Error::State)
        );
        assert!(!ledger.observed_after_dispatch(4));
    }
    Ok(())
}

#[test]
fn expanded_message_capacity_is_atomic_and_abort_always_closes() -> TestResult {
    for capacity in 1..=16 {
        let b = binding();
        let mut ledger = Ledger::new(b, capacity)?;
        let before = ledger.counts();
        assert_eq!(ledger.enqueue(b, 0, &[]), Err(Error::Sequence));
        assert_eq!(
            ledger.enqueue(b, 0, &vec![Recipient::Motion; capacity + 1]),
            Err(Error::Busy)
        );
        assert_eq!(
            ledger.enqueue(b, 1, &[Recipient::Task]),
            Err(Error::Sequence)
        );
        assert_eq!(ledger.counts(), before);
        ledger.enqueue(b, 0, &vec![Recipient::Task; capacity])?;
        assert_eq!(ledger.available(), 0);
        assert_eq!(ledger.enqueue(b, 1, &[Recipient::Task]), Err(Error::Busy));
        let ticket = ledger.begin_issue(1)?.ok_or("ticket")?;
        ledger.close();
        assert_eq!(
            ledger.acknowledge(ticket, Outcome::Accepted, 1),
            Err(Error::State)
        );
        assert_eq!(ledger.begin_issue(2), Err(Error::State));
        assert_eq!(ledger.counts().admitted_commands, 0);
    }
    Ok(())
}

#[test]
fn wrong_generation_piece_recipient_and_nonmonotonic_observations_are_rejected() -> TestResult {
    let b = binding();
    let mut ledger = Ledger::new(b, 4)?;
    ledger.enqueue(b, 0, &[Recipient::Motion])?;
    assert_eq!(ledger.begin_issue(0), Err(Error::Sequence));
    let ticket = ledger.begin_issue(10)?.ok_or("ticket")?;
    for kind in 0..6 {
        let mut wrong = ticket;
        match kind {
            0 => wrong.binding.generation.selection += 1,
            1 => wrong.binding.state_epoch += 1,
            2 => wrong.binding.artifact_sha256[0] ^= 1,
            3 => wrong.command += 1,
            4 => wrong.piece += 1,
            _ => wrong.recipient = Recipient::Task,
        }
        assert!(ledger.acknowledge(wrong, Outcome::Accepted, 10).is_err());
        assert_eq!(ledger.counts().admitted_commands, 0);
    }
    assert_eq!(
        ledger.acknowledge(ticket, Outcome::Accepted, 9),
        Err(Error::Sequence)
    );
    ledger.acknowledge(ticket, Outcome::Accepted, 10)?;
    assert_eq!(ledger.begin_issue(9), Err(Error::Sequence));
    Ok(())
}

#[test]
fn procedure_rebind_preserves_prefix_but_invalidates_old_receipts() -> TestResult {
    let b = binding();
    let next = Binding {
        state_epoch: 2,
        state_sha256: [6; 32],
        ..b
    };
    let mut ledger = Ledger::new(b, 4)?;
    ledger.enqueue(b, 0, &[Recipient::Io])?;
    assert_eq!(ledger.rebind(b, next), Err(Error::NotDrained));
    let ticket = ledger.begin_issue(1)?.ok_or("ticket")?;
    assert_eq!(ledger.rebind(b, next), Err(Error::NotDrained));
    ledger.acknowledge(ticket, Outcome::Accepted, 1)?;
    ledger.rebind(b, next)?;
    assert_eq!(
        ledger.acknowledge(ticket, Outcome::Accepted, 1),
        Err(Error::Stale)
    );
    assert_eq!(
        ledger.enqueue(b, 1, &[Recipient::Motion]),
        Err(Error::Stale)
    );
    ledger.enqueue(next, 1, &[Recipient::Motion])?;
    let ticket = ledger.begin_issue(2)?.ok_or("new binding")?;
    assert_eq!(ticket.binding, next);
    assert_eq!(ticket.serial, 2);
    assert_eq!(
        ledger.acknowledge(ticket, Outcome::Accepted, 2)?.admitted,
        2
    );
    Ok(())
}
