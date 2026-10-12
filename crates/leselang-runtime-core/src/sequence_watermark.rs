#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceWatermarkError {
    Exhausted,
    InvalidStored,
}

/// Mutable numeric observations, not a durable allocation or commit receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceWatermarkUpdate {
    pub next_sequence: u64,
    pub write_required: bool,
}

/// An explicit bounded requirement, not proof that an identity was allocated.
/// The private fields retain the original ceiling through reconciliation. Hosts
/// still own canonical identities, reservations, original allocation proof and
/// every graph/authority/receipt check in the same native snapshot/transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceWatermarkRequirement {
    required: u64,
    max_watermark: u64,
}

impl SequenceWatermarkRequirement {
    /// Calculate the next numeric requirement before observing stored metadata.
    /// Zero is an empty highest-sequence observation and requires watermark one;
    /// it does not become a valid identity or an allocation permission. The
    /// explicit inclusive watermark ceiling is not an effect-sequence ceiling.
    /// Overflow/ceiling refusal precedes native reads when called in that order.
    pub fn after_observation(
        used_sequence: u64,
        max_watermark: u64,
    ) -> Result<Self, SequenceWatermarkError> {
        let required = used_sequence
            .checked_add(1)
            .filter(|required| *required <= max_watermark)
            .ok_or(SequenceWatermarkError::Exhausted)?;
        Ok(Self {
            required,
            max_watermark,
        })
    }

    pub fn required_watermark(self) -> u64 {
        self.required
    }

    /// Reject zero/out-of-ceiling stored watermarks, then preserve existing gaps.
    /// Equal/higher stored values require no write; this never shrinks, fills a
    /// gap, clamps or wraps. The returned update is unchecked mutable data, not
    /// a reservation, authentic receipt, execution grant or commit certificate.
    /// A caller can supply a different stored observation on another call; there
    /// is no cache, synchronization, durable allocator or global registry here.
    ///
    /// All work is allocation-free numeric arithmetic without native hooks,
    /// codecs, identity normalization, reads/writes, fuel construction or replay.
    /// Hosts must validate the ORIGINAL allocation watermark before imported-ID
    /// repair, then apply any update inside the same original transaction and
    /// commit-before-publish. Planning does not make a failed write/commit safe.
    pub fn reconcile(
        self,
        stored_watermark: u64,
    ) -> Result<SequenceWatermarkUpdate, SequenceWatermarkError> {
        if stored_watermark == 0 || stored_watermark > self.max_watermark {
            return Err(SequenceWatermarkError::InvalidStored);
        }
        Ok(SequenceWatermarkUpdate {
            next_sequence: stored_watermark.max(self.required),
            write_required: self.required > stored_watermark,
        })
    }
}
