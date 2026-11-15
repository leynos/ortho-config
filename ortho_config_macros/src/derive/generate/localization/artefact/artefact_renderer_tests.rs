//! Renderer tests for the identifier artefact: ordering, the one MiB cap,
//! and the split-file round trip.

use super::*;

/// Verifies rendering sorts deterministically regardless of input order.
#[test]
fn renderer_sorts_entries_deterministically() -> Result<()> {
    let entries = vec![
        entry_with_type_name("zeta", String::from("Fixture")),
        entry_with_type_name("alpha", String::from("Fixture")),
        entry_with_type_name("middle", String::from("Fixture")),
    ];
    let first = render(entries.clone())?;
    let second = render(entries)?;
    let ids = round_trip_entries(&first)?
        .into_iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();

    ensure!(
        file_bytes(&first) == file_bytes(&second),
        "repeated renders must have identical file bytes"
    );
    ensure!(
        ids == ["alpha", "middle", "zeta"],
        "renderer must sort entries by stable schema order"
    );
    Ok(())
}

/// Verifies equal sort keys fall back to provenance rather than input order.
///
/// Two same-named types in different modules agree on every identity field the
/// renderer historically compared, so they are a genuine tie rather than a
/// contrived one: both declarations must survive, and only their source
/// locations separate them. Permuting the input is what exposes the tie — the
/// pre-existing determinism test renders one `Vec` twice and so cannot observe
/// that the order came from the input at all.
#[test]
fn renderer_breaks_provenance_ties_independently_of_input_order() -> Result<()> {
    // The two entries agree on every identity field and differ only here, so
    // the provenance value is written out rather than parameterised.
    let earlier = tied_fixture("b.rs", 40);
    let later = tied_fixture("b.rs", 80);
    let reversed = vec![later.clone(), earlier.clone()];

    let first = render(vec![earlier, later])?;
    let second = render(reversed)?;

    ensure!(
        file_bytes(&first) == file_bytes(&second),
        "equal-key entries must not take their order from the input"
    );
    ensure!(
        round_trip_entries(&first)?
            .into_iter()
            .map(|entry| (entry.source.file, entry.source.line))
            .collect::<Vec<_>>()
            == [(String::from("b.rs"), 40), (String::from("b.rs"), 80),],
        "the earlier declaration must sort first"
    );
    Ok(())
}

/// Builds the shared tie fixture at one provenance coordinate.
///
/// Both entries carry the same id and type name, because a tie is exactly the
/// case the ordering tie-breaker exists for: entries that agree on
/// `(id, kind, type_name, field)` and are separated only by where they were
/// declared.
fn tied_fixture(file: &str, line: usize) -> Entry {
    let source = Source {
        file: file.to_owned(),
        line,
        column: 2,
    };
    Entry {
        source,
        ..entry_with_type_name("tied", String::from("a::Fixture"))
    }
}

/// Verifies provenance ties also fix the split-file boundaries.
///
/// The single-file case has one byte sequence to compare; the split case has a
/// part count, a set of filenames, and a per-part assignment, each of which the
/// tie would otherwise leave to enumeration order. The payload rides on
/// `type_name`, which both entries must nevertheless share, so the bulk that
/// forces the split does not also break the tie being tested.
#[test]
fn split_renderer_breaks_provenance_ties_independently_of_input_order() -> Result<()> {
    let earlier = split_tied_fixture(10);
    let later = split_tied_fixture(70);
    let reversed = vec![later.clone(), earlier.clone()];

    let first = render(vec![earlier.clone(), later.clone()])?;
    let second = render(reversed)?;

    ensure!(
        first.len() > 1,
        "the fixture must be large enough to exercise the split path"
    );
    ensure!(
        file_bytes(&first) == file_bytes(&second),
        "split filenames and part contents must not vary with input order"
    );
    ensure!(
        round_trip_entries(&first)? == ordered(vec![earlier, later]),
        "split output must round-trip the provenance-ordered entries"
    );
    Ok(())
}

/// Builds the shared tie fixture large enough to force the split path.
///
/// The bulk rides on `type_name`, which both entries must nevertheless share:
/// a payload that also broke the tie would stop testing the tie-breaker.
fn split_tied_fixture(line: usize) -> Entry {
    let source = Source {
        file: String::from("c.rs"),
        line,
        column: 0,
    };
    Entry {
        source,
        ..entry_with_type_name(
            "tied",
            format!("Fixture{}", "x".repeat(SPLIT_PAYLOAD_BYTES)),
        )
    }
}

/// Verifies split output round-trips every ordered entry through its index.
#[test]
fn split_renderer_round_trips_ordered_entries() -> Result<()> {
    let entries = vec![
        entry_with_type_name("second", "x".repeat(SPLIT_PAYLOAD_BYTES)),
        entry_with_type_name("first", "x".repeat(SPLIT_PAYLOAD_BYTES)),
        entry_with_type_name("third", "x".repeat(SPLIT_PAYLOAD_BYTES)),
    ];
    let files = render(entries.clone())?;
    let round_tripped = round_trip_entries(&files)?;

    ensure!(
        files
            .first()
            .is_some_and(|file| file.name == "cli-identifiers.index.json"),
        "oversized document must begin with its split index"
    );
    ensure!(
        json(&round_tripped)? == json(&ordered(entries))?,
        "split artefact must round-trip every ordered entry"
    );
    Ok(())
}

/// Verifies the one MiB cap keeps boundary output whole and splits above it.
#[test]
fn renderer_honours_the_one_mebibyte_boundary() -> Result<()> {
    let at_cap = entry_at_cap()?;
    let above_cap = entry_with_type_name("boundary", format!("{}x", at_cap.type_name.as_str()));
    let at_cap_files = render(vec![at_cap])?;
    let above_cap_files = render(vec![above_cap])?;

    ensure!(
        at_cap_files
            .first()
            .is_some_and(|file| file.name == "cli-identifiers.json"),
        "a document exactly at the cap must remain unsplit"
    );
    ensure!(
        above_cap_files
            .first()
            .is_some_and(|file| file.name == "cli-identifiers.index.json"),
        "a document above the cap must use the split index"
    );
    Ok(())
}

/// Verifies one oversized entry remains available as a single indexed part.
#[test]
fn renderer_keeps_one_oversized_entry_in_a_single_part() -> Result<()> {
    let entry = entry_with_type_name("oversized", "x".repeat(CAP_BYTES));
    let files = render(vec![entry.clone()])?;
    let part = files
        .iter()
        .find(|file| file.name == "cli-identifiers.0.json")
        .context("single oversized entry part")?;

    ensure!(
        files.len() == 2,
        "single oversized entry must render only an index and one part"
    );
    ensure!(
        serde_json::from_slice::<Document>(&part.contents)?
            .entries
            .len()
            == 1,
        "single oversized entry part must retain the entry"
    );
    ensure!(
        json(&round_trip_entries(&files)?)? == json(&ordered(vec![entry]))?,
        "single oversized entry must round-trip through the index"
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Verifies split rendering round-trips arbitrary entries in every input order.
    #[test]
    fn split_renderer_round_trips_arbitrary_entries(
        entries in proptest::collection::vec(split_entry_strategy(), 5..8),
        offset in 0_usize..5,
    ) {
        let expected = ordered(entries.clone());
        let files = render(entries.clone()).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let round_tripped = round_trip_entries(&files)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let mut permuted = entries;
        permuted.rotate_left(offset);
        let permuted_files = render(permuted).map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert!(
            files.first().is_some_and(|file| file.name == "cli-identifiers.index.json"),
            "large generated inputs must use an index"
        );
        prop_assert_eq!(
            json(&round_tripped).map_err(|error| TestCaseError::fail(error.to_string()))?,
            json(&expected).map_err(|error| TestCaseError::fail(error.to_string()))?,
            "split output must round-trip every ordered entry"
        );
        prop_assert_eq!(
            file_bytes(&files),
            file_bytes(&permuted_files),
            "permuted input must render identical file bytes"
        );
        for file in &files {
            if file.name != "cli-identifiers.index.json" {
                let document = serde_json::from_slice::<Document>(&file.contents)
                    .map_err(|error| TestCaseError::fail(error.to_string()))?;
                prop_assert!(
                    file.contents.len() <= CAP_BYTES || document.entries.len() == 1,
                    "split parts must respect the cap unless one entry alone exceeds it"
                );
            }
        }
    }
}
