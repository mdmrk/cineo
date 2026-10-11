use anyhow::Context as _;
use librqbit::TorrentMetadata;

pub(crate) struct Layout {
    file_offsets: Vec<u64>,
    piece_length: u64,
    total_length: u64,
}

impl Layout {
    pub(crate) fn new(file_offsets: Vec<u64>, piece_length: u64, total_length: u64) -> Self {
        Self {
            file_offsets,
            piece_length,
            total_length,
        }
    }

    pub(crate) fn of(metadata: &TorrentMetadata) -> Self {
        let lengths = metadata.lengths();
        Self::new(
            metadata
                .file_infos
                .iter()
                .map(|f| f.offset_in_torrent)
                .collect(),
            u64::from(lengths.default_piece_length()),
            lengths.total_length(),
        )
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "offsets within one piece, a u32 in librqbit"
    )]
    pub(crate) fn parts(
        &self,
        file_id: usize,
        offset: u64,
        len: usize,
    ) -> anyhow::Result<impl Iterator<Item = (u64, usize, usize)> + use<>> {
        let start = self
            .file_offsets
            .get(file_id)
            .with_context(|| format!("no file {file_id} in this torrent"))?
            + offset;
        let end = start + len as u64;
        anyhow::ensure!(end <= self.total_length, "past the end of the torrent");
        let piece_length = self.piece_length;
        let mut at = start;
        Ok(std::iter::from_fn(move || {
            (at < end).then(|| {
                let piece = at / piece_length;
                let in_piece = at % piece_length;
                let part = (piece_length - in_piece).min(end - at);
                at += part;
                (piece, in_piece as usize, part as usize)
            })
        }))
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "a piece length, a u32 in librqbit"
    )]
    pub(crate) fn piece_len(&self, piece: u64) -> usize {
        (self.total_length - piece * self.piece_length).min(self.piece_length) as usize
    }
}
