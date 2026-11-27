package io.github.mdmrk.cineo.ui

import android.content.pm.ActivityInfo
import android.content.res.Configuration
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerOrientationTest {
    @Test
    fun aPortraitPlayerTurnsToLandscape() {
        assertEquals(
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE,
            rotatedOrientation(Configuration.ORIENTATION_PORTRAIT),
        )
    }

    @Test
    fun aLandscapePlayerTurnsToPortrait() {
        assertEquals(
            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT,
            rotatedOrientation(Configuration.ORIENTATION_LANDSCAPE),
        )
    }

    @Test
    fun largeScreensFromAndroid16OnCannotBeRotatedByTheApp() {
        assertTrue(canRotate(sdk = 35, smallestWidthDp = 800))
        assertTrue(canRotate(sdk = 37, smallestWidthDp = 411))
        assertFalse(canRotate(sdk = 36, smallestWidthDp = 600))
        assertFalse(canRotate(sdk = 37, smallestWidthDp = 800))
    }
}
