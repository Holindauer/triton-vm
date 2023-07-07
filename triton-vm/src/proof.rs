use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use get_size::GetSize;
use serde::Deserialize;
use serde::Serialize;
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::bfield_codec::BFieldCodec;
use twenty_first::shared_math::tip5::Digest;

use crate::proof_stream::ProofStream;
use crate::stark;

/// Contains the necessary cryptographic information to verify a computation.
/// Should be used together with a [`Claim`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, BFieldCodec)]
pub struct Proof(pub Vec<BFieldElement>);

impl GetSize for Proof {
    fn get_stack_size() -> usize {
        std::mem::size_of::<Self>()
    }

    fn get_heap_size(&self) -> usize {
        self.0.len() * std::mem::size_of::<BFieldElement>()
    }
}

impl Proof {
    /// Get the height of the trace used during proof generation.
    /// This is an upper bound on the length of the computation this proof is for.
    /// It it one of the main contributing factors to the length of the FRI domain.
    pub fn padded_height(&self) -> Result<usize> {
        let proof_stream = ProofStream::<stark::StarkHasher>::try_from(self)?;
        let mut padded_height = None;
        for item in proof_stream.items {
            if let Ok(log_2_padded_height) = item.as_log2_padded_height() {
                match padded_height.is_some() {
                    true => bail!("The proof must contain at most one log_2_padded_height."),
                    false => padded_height = Some(1 << log_2_padded_height),
                }
            }
        }
        padded_height.ok_or(anyhow!("The proof must contain a log_2_padded_height."))
    }
}

/// Contains the public information of a verifiably correct computation.
/// A corresponding [`Proof`] is needed to verify the computation.
/// One additional piece of public information not explicitly listed in the [`Claim`] is the
/// `padded_height`, an upper bound on the length of the computation.
/// It is derivable from a [`Proof`] by calling [`Proof::padded_height()`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, GetSize, BFieldCodec, Hash)]
pub struct Claim {
    /// The hash digest of the program that was executed. The hash function in use is Tip5.
    pub program_digest: Digest,

    /// The public input to the computation.
    pub input: Vec<BFieldElement>,

    /// The public output of the computation.
    pub output: Vec<BFieldElement>,
}

impl Claim {
    /// The public input as `u64`s.
    /// If `BFieldElement`s are needed, use field `input`.
    pub fn public_input(&self) -> Vec<u64> {
        self.input.iter().map(|x| x.value()).collect()
    }

    /// The public output as `u64`.
    /// If `BFieldElements`s are needed, use field `output`.
    pub fn public_output(&self) -> Vec<u64> {
        self.output.iter().map(|x| x.value()).collect()
    }
}

#[cfg(test)]
pub mod test_claim_proof {
    use rand::random;
    use twenty_first::shared_math::b_field_element::BFieldElement;
    use twenty_first::shared_math::bfield_codec::BFieldCodec;
    use twenty_first::shared_math::other::random_elements;

    use crate::proof_item::ProofItem;
    use crate::stark::StarkHasher;

    use super::*;

    #[test]
    fn test_decode_proof() {
        let data: Vec<BFieldElement> = random_elements(348);
        let proof = Proof(data);

        let encoded = proof.encode();
        let decoded = *Proof::decode(&encoded).unwrap();

        assert_eq!(proof, decoded);
    }

    #[test]
    fn test_decode_claim() {
        let claim = Claim {
            program_digest: random(),
            input: random_elements(346),
            output: random_elements(125),
        };

        let encoded = claim.encode();
        let decoded = *Claim::decode(&encoded).unwrap();

        assert_eq!(claim.program_digest, decoded.program_digest);
        assert_eq!(claim.input, decoded.input);
        assert_eq!(claim.output, decoded.output);
    }

    #[test]
    fn proof_with_no_log_2_padded_height_gives_err() {
        let mut proof_stream = ProofStream::<StarkHasher>::new();
        proof_stream.enqueue(&ProofItem::MerkleRoot(random()));
        let proof: Proof = proof_stream.into();
        let maybe_padded_height = proof.padded_height();
        assert!(maybe_padded_height.is_err());
    }

    #[test]
    fn proof_with_multiple_log_2_padded_height_gives_err() {
        let mut proof_stream = ProofStream::<StarkHasher>::new();
        proof_stream.enqueue(&ProofItem::Log2PaddedHeight(8));
        proof_stream.enqueue(&ProofItem::MerkleRoot(random()));
        proof_stream.enqueue(&ProofItem::Log2PaddedHeight(7));
        let proof: Proof = proof_stream.into();
        let maybe_padded_height = proof.padded_height();
        assert!(maybe_padded_height.is_err());
    }
}
