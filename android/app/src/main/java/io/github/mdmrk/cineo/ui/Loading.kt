package io.github.mdmrk.cineo.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.withInfiniteAnimationFrameMillis
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.imageResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.ArtView
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.roundToInt

private const val LOADING_W = 328f
private const val LOADING_H = 309f
private const val REEL_PIVOT_X = 192.9f
private const val REEL_PIVOT_Y = 192.0f
private const val REEL_PERIOD = 5.0
private const val MOUSTACHE_PIVOT_X = 135.0f
private const val MOUSTACHE_PIVOT_Y = 280.535f
private const val MOUSTACHE_PERIOD = 2.5
private val MOUSTACHE_KEYS = listOf(0.0 to 0f, 0.07 to 5f, 0.15 to 0f, 0.22 to 5f, 0.31 to 0f, 1.0 to 0f)
private const val STRIP_X = 218f
private const val STRIP_Y = 134f
private const val STRIP_W = 108f
private const val STRIP_H = 164f
private const val STRIP_FRAMES = 10
private const val STRIP_COLUMNS = 5
private const val STRIP_PERIOD = 1.0 / 3.0
private const val PULSE_PERIOD = 1600

val LoadingWidth = 128.dp
val PlayerLoadingWidth = 176.dp
private val LoadingBackdropDim = Color.Black.copy(alpha = 1f - 70f / 255f)

private class Layers(
    val base: ImageBitmap,
    val reel: ImageBitmap,
    val left: ImageBitmap,
    val right: ImageBitmap,
    val strip: ImageBitmap,
)

@Composable
fun CineoLoading(modifier: Modifier = Modifier, width: Dp = LoadingWidth, tint: Color = Brand.TextDim) {
    val layers = Layers(
        ImageBitmap.imageResource(R.drawable.loading_base),
        ImageBitmap.imageResource(R.drawable.loading_reel),
        ImageBitmap.imageResource(R.drawable.loading_moustache_left),
        ImageBitmap.imageResource(R.drawable.loading_moustache_right),
        ImageBitmap.imageResource(R.drawable.loading_strip),
    )
    var millis by remember { mutableLongStateOf(0L) }
    LaunchedEffect(Unit) {
        val start = withInfiniteAnimationFrameMillis { it }
        while (true) withInfiniteAnimationFrameMillis { millis = it - start }
    }
    val filter = remember(tint) { ColorFilter.tint(tint, BlendMode.Modulate) }
    Canvas(modifier.width(width).aspectRatio(LOADING_W / LOADING_H)) {
        paintLoading(layers, millis / 1000.0, filter)
    }
}

private fun DrawScope.paintLoading(layers: Layers, time: Double, filter: ColorFilter) {
    val scale = size.width / LOADING_W
    val whole = IntSize(size.width.roundToInt(), size.height.roundToInt())
    fun layer(image: ImageBitmap) = drawImage(
        image,
        srcSize = IntSize(image.width, image.height),
        dstSize = whole,
        colorFilter = filter,
    )
    layer(layers.base)
    rotate(reelAngle(time), Offset(REEL_PIVOT_X * scale, REEL_PIVOT_Y * scale)) { layer(layers.reel) }
    val wiggle = moustacheAngle(time)
    val pivot = Offset(MOUSTACHE_PIVOT_X * scale, MOUSTACHE_PIVOT_Y * scale)
    rotate(wiggle, pivot) { layer(layers.left) }
    rotate(-wiggle, pivot) { layer(layers.right) }
    val frame = stripFrame(time)
    val rows = (STRIP_FRAMES + STRIP_COLUMNS - 1) / STRIP_COLUMNS
    val cellW = layers.strip.width / STRIP_COLUMNS
    val cellH = layers.strip.height / rows
    drawImage(
        layers.strip,
        srcOffset = IntOffset((frame % STRIP_COLUMNS) * cellW, (frame / STRIP_COLUMNS) * cellH),
        srcSize = IntSize(cellW, cellH),
        dstOffset = IntOffset((STRIP_X * scale).roundToInt(), (STRIP_Y * scale).roundToInt()),
        dstSize = IntSize((STRIP_W * scale).roundToInt(), (STRIP_H * scale).roundToInt()),
        colorFilter = filter,
    )
}

private fun fract(value: Double) = value - kotlin.math.floor(value)

private fun reelAngle(time: Double) = (fract(time / REEL_PERIOD) * 360.0).toFloat()

private fun moustacheAngle(time: Double): Float {
    val phase = fract(time / MOUSTACHE_PERIOD)
    val index = MOUSTACHE_KEYS.indices.drop(1).firstOrNull { phase < MOUSTACHE_KEYS[it].first } ?: return 0f
    val (t0, from) = MOUSTACHE_KEYS[index - 1]
    val (t1, to) = MOUSTACHE_KEYS[index]
    val u = ((phase - t0) / (t1 - t0)).toFloat()
    return from + (to - from) * u * u * (3f - 2f * u)
}

private fun stripFrame(time: Double) = (fract(time / STRIP_PERIOD) * STRIP_FRAMES).toInt().coerceAtMost(STRIP_FRAMES - 1)

@Composable
fun LoadingBackdrop(art: ArtView?, images: Images, firstLoad: Boolean, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        if (firstLoad && art?.background != null) {
            NetImage(images, art.background, 1280.dp, Modifier.fillMaxSize())
            Box(Modifier.fillMaxSize().background(LoadingBackdropDim))
        }
        val title = art?.title?.trim().orEmpty()
        when {
            firstLoad && (art?.logo != null || title.isNotEmpty()) -> LoadingArt(title, art?.logo, images)
            else -> CineoLoading(width = PlayerLoadingWidth, tint = Brand.TextBright)
        }
    }
}

@Composable
private fun LoadingArt(title: String, logo: String?, images: Images) {
    val pulse = rememberInfiniteTransition(label = "loading-art")
    val phase by pulse.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(PULSE_PERIOD), RepeatMode.Restart),
        label = "pulse",
    )
    val opacity = 0.25f + 0.75f * (0.5f - 0.5f * cos(phase * 2 * PI).toFloat())
    Box(Modifier.fillMaxSize().graphicsLayer { alpha = opacity }, contentAlignment = Alignment.Center) {
        if (logo != null) {
            NetImage(
                images,
                logo,
                360.dp,
                Modifier.fillMaxWidth(0.5f).fillMaxHeight(0.3f).widthIn(max = 560.dp).heightIn(max = 200.dp),
                contentScale = ContentScale.Fit,
            )
        } else {
            Text(
                title,
                color = Brand.TextBright,
                style = MaterialTheme.typography.headlineLarge.copy(fontSize = 36.sp, lineHeight = 42.sp),
                textAlign = TextAlign.Center,
                maxLines = 3,
                modifier = Modifier.fillMaxWidth(0.8f),
            )
        }
    }
}
