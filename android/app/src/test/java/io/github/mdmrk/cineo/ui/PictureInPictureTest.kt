package io.github.mdmrk.cineo.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class PictureInPictureTest {
    @Test
    fun theWindowTakesTheShapeOfTheVideo() {
        assertEquals(1920 to 1080, pipAspect(1920, 1080))
        assertEquals(1080 to 1920, pipAspect(1080, 1920))
    }

    @Test
    fun aShapeAndroidWouldRefuseIsNarrowedToItsLimit() {
        assertEquals(239 to 100, pipAspect(3840, 1000))
        assertEquals(100 to 239, pipAspect(500, 2000))
    }

    @Test
    fun aVideoWithoutASizeYetHasNoShape() {
        assertEquals(null, pipAspect(0, 0))
        assertEquals(null, videoBounds(1080, 2340, 0, 0))
    }

    @Test
    fun theVideoIsCentredInTheScreenWithBarsOnTheLongSide() {
        assertEquals(Bounds(0, 866, 1080, 1474), videoBounds(1080, 2340, 1920, 1080))
        assertEquals(Bounds(133, 0, 2207, 1080), videoBounds(2340, 1080, 1920, 1000))
    }
}
