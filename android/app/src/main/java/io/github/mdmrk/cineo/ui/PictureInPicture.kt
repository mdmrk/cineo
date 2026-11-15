package io.github.mdmrk.cineo.ui

import android.app.Activity
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.pm.PackageManager
import android.graphics.Rect
import android.graphics.drawable.Icon
import android.os.Build
import android.util.Rational
import io.github.mdmrk.cineo.PlaybackService
import io.github.mdmrk.cineo.PlaybackSession
import io.github.mdmrk.cineo.R
import kotlin.math.roundToInt

private const val WIDEST = 2.39

internal fun pipAspect(width: Int, height: Int): Pair<Int, Int>? {
    if (width <= 0 || height <= 0) return null
    val ratio = width.toDouble() / height
    return when {
        ratio > WIDEST -> 239 to 100
        ratio < 1 / WIDEST -> 100 to 239
        else -> width to height
    }
}

internal data class Bounds(val left: Int, val top: Int, val right: Int, val bottom: Int)

internal fun videoBounds(viewWidth: Int, viewHeight: Int, width: Int, height: Int): Bounds? {
    if (viewWidth <= 0 || viewHeight <= 0 || width <= 0 || height <= 0) return null
    val scale = minOf(viewWidth.toDouble() / width, viewHeight.toDouble() / height)
    val shownWidth = (width * scale).roundToInt()
    val shownHeight = (height * scale).roundToInt()
    val left = (viewWidth - shownWidth) / 2
    val top = (viewHeight - shownHeight) / 2
    return Bounds(left, top, left + shownWidth, top + shownHeight)
}

fun Activity.canPictureInPicture(): Boolean =
    packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)

data class Pip(val auto: Boolean, val width: Int, val height: Int, val paused: Boolean, val step: Int) {
    companion object {
        val Off = Pip(auto = false, width = 0, height = 0, paused = true, step = 0)
    }
}

fun Activity.pictureInPictureParams(pip: Pip): PictureInPictureParams {
    val (auto, width, height, paused, step) = pip
    val action = { icon: Int, label: Int, name: String ->
        RemoteAction(
            Icon.createWithResource(this, icon),
            getString(label),
            getString(label),
            PlaybackService.order(this, name),
        )
    }
    val builder = PictureInPictureParams.Builder().setActions(
        listOf(
            action(PlaybackSession.backIcon(step), R.string.seek_back_notification, PlaybackService.REWIND),
            if (paused) action(R.drawable.media_play, R.string.play, PlaybackService.TOGGLE)
            else action(R.drawable.media_pause, R.string.pause, PlaybackService.TOGGLE),
            action(PlaybackSession.forwardIcon(step), R.string.seek_forward_notification, PlaybackService.FORWARD),
        ),
    )
    pipAspect(width, height)?.let { (w, h) -> builder.setAspectRatio(Rational(w, h)) }
    val view = window.decorView
    videoBounds(view.width, view.height, width, height)?.let {
        builder.setSourceRectHint(Rect(it.left, it.top, it.right, it.bottom))
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        builder.setAutoEnterEnabled(auto)
        builder.setSeamlessResizeEnabled(true)
    }
    return builder.build()
}
