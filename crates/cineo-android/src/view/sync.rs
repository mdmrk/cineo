use cineo_core::app::{SyncFailure, SyncProblem, SyncState};
use cineo_core::sync::DEFAULT_SERVER;
use cineo_net::sync::{format_code, pairing_link};
use qrcode::{Color, QrCode};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncView {
    pub code: String,
    pub qr: Option<QrView>,
    pub status: SyncStatusView,
    pub can_sync_now: bool,
    pub server: String,
    pub own_server: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct QrView {
    pub width: u32,
    pub dark: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SyncStatusView {
    Syncing,
    NotYet,
    Synced { minutes_ago: u64 },
    KeyChanged,
    Busy,
    Failed { reason: SyncFailureView },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SyncFailureView {
    Insecure,
    Unreachable,
    Rejected { status: u16 },
    Unreadable,
    Conflict,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncLinkView {
    pub code: String,
    pub server: String,
}

fn qr(text: &str) -> Option<QrView> {
    let code = QrCode::new(text.as_bytes()).ok()?;
    Some(QrView {
        width: u32::try_from(code.width()).ok()?,
        dark: code
            .to_colors()
            .into_iter()
            .map(|c| c == Color::Dark)
            .collect(),
    })
}

fn status(now_ms: u64, sync: &SyncState) -> SyncStatusView {
    if sync.is_running() {
        return SyncStatusView::Syncing;
    }
    match &sync.problem {
        Some(SyncProblem::SpaceGone) => SyncStatusView::KeyChanged,
        Some(SyncProblem::Busy) => SyncStatusView::Busy,
        Some(SyncProblem::Failed(failure)) => SyncStatusView::Failed {
            reason: match failure {
                SyncFailure::Insecure => SyncFailureView::Insecure,
                SyncFailure::Network(_) => SyncFailureView::Unreachable,
                SyncFailure::Rejected(status) => SyncFailureView::Rejected { status: *status },
                SyncFailure::Malformed => SyncFailureView::Unreadable,
                SyncFailure::Conflict(_) => SyncFailureView::Conflict,
                SyncFailure::SpaceGone | SyncFailure::Unavailable { .. } => {
                    SyncFailureView::Unavailable
                }
            },
        },
        None => match sync.last_ok_ms {
            None => SyncStatusView::NotYet,
            Some(at) => SyncStatusView::Synced {
                minutes_ago: now_ms.saturating_sub(at) / 60_000,
            },
        },
    }
}

pub(super) fn sync(now_ms: u64, sync: &SyncState) -> SyncView {
    SyncView {
        code: format_code(&sync.pairing.key),
        qr: qr(&pairing_link(&sync.pairing)),
        status: status(now_ms, sync),
        can_sync_now: sync.can_sync_now(),
        server: sync.pairing.server.to_string(),
        own_server: sync.pairing.server.as_str() != DEFAULT_SERVER,
    }
}
