//! Random codes people read and type: the setup code (#028), recovery codes (#047), and
//! temporary passwords (#046).
//!
//! Characters come from a 32-character alphabet without look-alikes (no `0`/`O`, `1`/`I`),
//! shown in groups of 4: `K7QM-4XRT-9PWD-H2CN`.

/// The characters codes use: 32 of them, so each random byte maps to one without bias.
pub const ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// `chars` random characters from [`ALPHABET`].
///
/// # Errors
///
/// [`getrandom::Error`] if the operating system has no random bytes.
pub fn random_code(chars: usize) -> Result<String, getrandom::Error> {
    let mut bytes = vec![0_u8; chars];
    getrandom::fill(&mut bytes)?;
    // 256 is a multiple of 32, so `byte % 32` picks every character equally often.
    Ok(bytes
        .iter()
        .map(|byte| char::from(ALPHABET[usize::from(*byte) % ALPHABET.len()]))
        .collect())
}

/// `K7QM4XRT9PWDH2CN` → `K7QM-4XRT-9PWD-H2CN`.
#[must_use]
pub fn group_by_four(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_use_only_unambiguous_characters_and_are_random() {
        let code = random_code(16).unwrap();
        assert_eq!(code.len(), 16);
        assert!(code.bytes().all(|byte| ALPHABET.contains(&byte)), "{code}");
        assert!(!code.contains(['0', 'O', '1', 'I']));
        assert_ne!(random_code(16).unwrap(), code);
    }

    #[test]
    fn codes_are_shown_in_groups_of_four() {
        assert_eq!(group_by_four("K7QM4XRT9PWDH2CN"), "K7QM-4XRT-9PWD-H2CN");
        assert_eq!(group_by_four("QX7MK2PD9WRT"), "QX7M-K2PD-9WRT");
    }
}
