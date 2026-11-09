package io.github.mdmrk.cineo.ui

import android.content.pm.ActivityInfo
import android.content.res.Configuration
import org.junit.Assert.assertEquals
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
}
