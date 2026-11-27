package io.github.mdmrk.cineo.ui

import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.os.Build

private const val LARGE_SCREEN_DP = 600

fun canRotate(sdk: Int, smallestWidthDp: Int): Boolean =
    sdk < Build.VERSION_CODES.BAKLAVA || smallestWidthDp < LARGE_SCREEN_DP

fun rotatedOrientation(current: Int): Int =
    if (current == Configuration.ORIENTATION_LANDSCAPE) {
        ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
    } else {
        ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
    }
