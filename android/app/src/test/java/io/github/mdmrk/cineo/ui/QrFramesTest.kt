package io.github.mdmrk.cineo.ui

import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class QrFramesTest {
    private val link = "cineo:///sync/AAAQ-EAYE-AUDA-OCAJ-BIFQ-YDIO-B4KA-47NG"

    private fun frame(text: String, size: Int, rowStride: Int, offset: Int, turned: Boolean): ByteArray {
        val matrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size / 2, size / 2)
        val frame = ByteArray(rowStride * size) { 0x55 }
        for (y in 0 until size) {
            for (x in 0 until size) frame[y * rowStride + x] = 0xC8.toByte()
        }
        for (y in 0 until matrix.height) {
            for (x in 0 until matrix.width) {
                val (fx, fy) = if (turned) matrix.height - 1 - y to x else x to y
                val dark = matrix.get(x, y)
                frame[(fy + offset) * rowStride + fx + offset] = if (dark) 0x14 else 0xC8.toByte()
            }
        }
        return frame
    }

    @Test
    fun aQrCodeIsReadFromTheLuminancePlaneWithItsRowPadding() {
        val frames = QrFrames()
        assertEquals(link, frames.read(frame(link, 400, 448, 60, turned = false), 400, 400, 448))
        assertEquals(link, frames.read(frame(link, 400, 400, 100, turned = true), 400, 400, 400))
    }

    @Test
    fun aFrameWithoutAQrCodeReadsNothing() {
        val blank = ByteArray(320 * 240) { 0x80.toByte() }
        assertNull(QrFrames().read(blank, 320, 240, 320))
    }
}
