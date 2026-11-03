package io.github.mdmrk.cineo.ui

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.ImageException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private const val MAX_SIDE = 4096
private const val CACHE_BYTES = 32 * 1024 * 1024

class Images(private val cineo: Cineo) {
    private val cache = object : LruCache<String, ImageBitmap>(CACHE_BYTES) {
        override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
    }

    suspend fun load(url: String, width: Int): ImageBitmap? {
        cache.get(url)?.let { return it }
        val bytes = try {
            cineo.fetchImage(url)
        } catch (e: ImageException) {
            return null
        }
        val image = withContext(Dispatchers.Default) { decode(bytes, width) } ?: return null
        cache.put(url, image)
        return image
    }
}

private fun decode(bytes: ByteArray, width: Int): ImageBitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    if (bounds.outWidth > MAX_SIDE || bounds.outHeight > MAX_SIDE) return null
    var sample = 1
    while (bounds.outWidth / (sample * 2) >= width) sample *= 2
    val options = BitmapFactory.Options().apply { inSampleSize = sample }
    return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap()
}
