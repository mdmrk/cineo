package io.github.mdmrk.cineo.ui

import android.content.pm.ActivityInfo
import android.content.res.Configuration

fun rotatedOrientation(current: Int): Int =
    if (current == Configuration.ORIENTATION_LANDSCAPE) {
        ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
    } else {
        ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
    }
