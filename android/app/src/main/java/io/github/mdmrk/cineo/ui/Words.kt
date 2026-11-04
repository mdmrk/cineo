package io.github.mdmrk.cineo.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.NoticeView
import io.github.mdmrk.cineo.rust.PlaybackFailureView
import io.github.mdmrk.cineo.rust.ProblemView
import io.github.mdmrk.cineo.rust.RowView
import io.github.mdmrk.cineo.rust.SourceKindView

@Composable
fun problemText(problem: ProblemView): String = when (problem) {
    is ProblemView.AlreadyInstalled -> stringResource(R.string.problem_already_installed)
    is ProblemView.ConfigurationRequired -> stringResource(R.string.problem_configuration_required)
    is ProblemView.InvalidAddonUrl -> stringResource(R.string.problem_invalid_url, problem.reason)
    is ProblemView.NoMetaAddon -> stringResource(R.string.problem_no_meta)
    is ProblemView.UnsupportedFilter -> stringResource(R.string.problem_unsupported_filter)
    is ProblemView.Request -> problem.reason
}

@Composable
fun noticeText(notice: NoticeView): String = when (notice) {
    is NoticeView.AddonUnavailable -> stringResource(R.string.notice_addon_unavailable, notice.reason)
    is NoticeView.UnsupportedScheme -> stringResource(R.string.notice_unsupported_scheme, notice.scheme)
    is NoticeView.TorrentsOff -> stringResource(R.string.notice_torrents_off)
    is NoticeView.UnsupportedSource ->
        stringResource(R.string.notice_unsupported_source, sourceName(notice.kind))
    is NoticeView.PlaybackFailed -> {
        val reason = when (val failure = notice.failure) {
            is PlaybackFailureView.Player -> failure.reason
            is PlaybackFailureView.Torrent -> stringResource(R.string.notice_torrent_failed, failure.reason)
        }
        if (notice.pickAnother) stringResource(R.string.notice_pick_another, reason) else reason
    }
    is NoticeView.UnexpectedEngineAddress -> stringResource(R.string.notice_engine_address)
    is NoticeView.SubtitleFailed -> stringResource(R.string.notice_subtitle_failed, notice.reason)
    is NoticeView.SaveFailed -> stringResource(R.string.notice_save_failed, notice.reason)
    is NoticeView.StoreClosed -> stringResource(R.string.notice_store_closed)
    is NoticeView.InvalidLink -> stringResource(R.string.notice_invalid_link)
}

@Composable
private fun sourceName(kind: SourceKindView): String = stringResource(
    when (kind) {
        SourceKindView.HTTP -> R.string.source_http
        SourceKindView.OTHER_URL -> R.string.source_url
        SourceKindView.YOU_TUBE -> R.string.source_youtube
        SourceKindView.TORRENT -> R.string.source_torrent
        SourceKindView.EXTERNAL -> R.string.source_external
        SourceKindView.ARCHIVE -> R.string.source_archive
        SourceKindView.USENET -> R.string.source_usenet
    },
)

@Composable
fun catalogTitle(row: RowView): String {
    val kind = when (row.contentType) {
        "movie" -> stringResource(R.string.type_movie)
        "series" -> stringResource(R.string.type_series)
        "channel" -> stringResource(R.string.type_channel)
        "tv" -> stringResource(R.string.type_tv)
        else -> row.contentType
    }
    return stringResource(R.string.catalog_title, row.name, kind)
}

@Composable
fun noticeParts(notice: NoticeView): Pair<String, String?> = when (notice) {
    is NoticeView.AddonUnavailable -> stringResource(R.string.notice_addon_unavailable_short) to notice.reason
    is NoticeView.PlaybackFailed -> {
        val title = when {
            notice.pickAnother -> R.string.notice_pick_another_short
            notice.failure is PlaybackFailureView.Torrent -> R.string.notice_torrent_failed_short
            else -> R.string.notice_playback_failed_short
        }
        val reason = when (val failure = notice.failure) {
            is PlaybackFailureView.Player -> failure.reason
            is PlaybackFailureView.Torrent -> failure.reason
        }
        stringResource(title) to reason
    }
    is NoticeView.SubtitleFailed -> stringResource(R.string.notice_subtitle_failed_short) to notice.reason
    is NoticeView.SaveFailed -> stringResource(R.string.notice_save_failed_short) to notice.reason
    else -> noticeText(notice) to null
}
