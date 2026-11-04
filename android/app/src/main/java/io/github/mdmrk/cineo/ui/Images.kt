package io.github.mdmrk.cineo.ui

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.ImageException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private const val MAX_SIDE = 4096
private const val CACHE_BYTES = 32 * 1024 * 1024

internal class Decoded<T>(val image: T, val width: Int, val full: Boolean) {
    fun serves(wanted: Int) = full || width >= wanted
}

class Images(private val cineo: Cineo) {
    private val cache = object : LruCache<String, Decoded<ImageBitmap>>(CACHE_BYTES) {
        override fun sizeOf(key: String, value: Decoded<ImageBitmap>) =
            value.image.width * value.image.height * 4
    }

    suspend fun load(url: String, width: Int): ImageBitmap? {
        cache.get(url)?.takeIf { it.serves(width) }?.let { return it.image }
        val bytes = try {
            cineo.fetchImage(url)
        } catch (e: ImageException) {
            return null
        }
        val decoded = withContext(Dispatchers.Default) { decode(bytes, width) } ?: return null
        if (cache.get(url)?.serves(decoded.width) != true) cache.put(url, decoded)
        return decoded.image
    }
}

private fun decode(bytes: ByteArray, width: Int): Decoded<ImageBitmap>? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    if (bounds.outWidth > MAX_SIDE || bounds.outHeight > MAX_SIDE) return null
    var sample = 1
    while (bounds.outWidth / (sample * 2) >= width) sample *= 2
    val options = BitmapFactory.Options().apply { inSampleSize = sample }
    val image = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap() ?: return null
    return Decoded(image, image.width, full = sample == 1)
}

@Composable
fun NetImage(
    images: Images,
    url: String?,
    width: Dp,
    modifier: Modifier = Modifier,
    contentDescription: String? = null,
    contentScale: ContentScale = ContentScale.Crop,
    alignment: Alignment = Alignment.Center,
) {
    val widthPx = with(LocalDensity.current) { width.roundToPx() }
    var image by remember(url) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(url, widthPx) {
        image = url?.let { images.load(it, widthPx) }
    }
    val alpha by animateFloatAsState(if (image != null) 1f else 0f, label = "image")
    image?.let {
        Image(
            bitmap = it,
            contentDescription = contentDescription,
            contentScale = contentScale,
            alignment = alignment,
            modifier = modifier.graphicsLayer { this.alpha = alpha },
        )
    }
}

@Composable
fun Placeholder(modifier: Modifier = Modifier) {
    val pulse = rememberInfiniteTransition(label = "placeholder")
    val alpha by pulse.animateFloat(
        initialValue = 0.5f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(800), RepeatMode.Reverse),
        label = "pulse",
    )
    Box(modifier.graphicsLayer { this.alpha = alpha }.background(Brand.Surface))
}
