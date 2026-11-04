package io.github.mdmrk.cineo.ui

import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.mdmrk.cineo.R

object Brand {
    val Background = Color(0xFF14181D)
    val Panel = Color(0xFF1B2128)
    val Surface = Color(0xFF252D36)
    val SurfaceHover = Color(0xFF313B46)
    val Rule = Color(0xFF262E37)
    val TextBright = Color(0xFFF1F4F7)
    val Text = Color(0xFFC3CCD6)
    val TextDim = Color(0xFF8B98A8)
    val TextFaint = Color(0xFF606C7B)
    val Accent = Color(0xFF34C77B)
    val OnAccent = Color(0xFF06180E)
    val Danger = Color(0xFFFF6B6B)
    val Warning = Color(0xFFF2A14B)
}

val Inter = FontFamily(
    Font(R.font.inter_regular, FontWeight.Normal),
    Font(R.font.inter_semibold, FontWeight.SemiBold),
)

private val TablerRegular = FontFamily(Font(R.font.tabler_regular))
private val TablerFilled = FontFamily(Font(R.font.tabler_filled))

private val colors = darkColorScheme(
    primary = Brand.Accent,
    onPrimary = Brand.OnAccent,
    primaryContainer = Brand.Accent,
    onPrimaryContainer = Brand.OnAccent,
    secondary = Brand.Text,
    onSecondary = Brand.Background,
    secondaryContainer = Brand.Surface,
    onSecondaryContainer = Brand.TextBright,
    background = Brand.Background,
    onBackground = Brand.Text,
    surface = Brand.Background,
    onSurface = Brand.TextBright,
    surfaceVariant = Brand.Surface,
    onSurfaceVariant = Brand.TextDim,
    surfaceContainerLowest = Brand.Background,
    surfaceContainerLow = Brand.Panel,
    surfaceContainer = Brand.Panel,
    surfaceContainerHigh = Brand.Surface,
    surfaceContainerHighest = Brand.SurfaceHover,
    inverseSurface = Brand.Surface,
    inverseOnSurface = Brand.TextBright,
    inversePrimary = Brand.Accent,
    outline = Brand.TextFaint,
    outlineVariant = Brand.Rule,
    error = Brand.Danger,
    onError = Brand.Background,
)

private fun TextStyle.inter(weight: FontWeight = FontWeight.Normal) =
    copy(fontFamily = Inter, fontWeight = weight)

private val base = Typography()

private val typography = Typography(
    displaySmall = base.displaySmall.inter(FontWeight.SemiBold),
    headlineLarge = base.headlineLarge.inter(FontWeight.SemiBold),
    headlineMedium = base.headlineMedium.inter(FontWeight.SemiBold),
    headlineSmall = base.headlineSmall.inter(FontWeight.SemiBold),
    titleLarge = base.titleLarge.inter(FontWeight.SemiBold),
    titleMedium = base.titleMedium.inter(FontWeight.SemiBold),
    titleSmall = base.titleSmall.inter(FontWeight.SemiBold),
    bodyLarge = base.bodyLarge.inter(),
    bodyMedium = base.bodyMedium.inter(),
    bodySmall = base.bodySmall.inter(),
    labelLarge = base.labelLarge.inter(FontWeight.SemiBold),
    labelMedium = base.labelMedium.inter(FontWeight.SemiBold),
    labelSmall = base.labelSmall.inter(FontWeight.SemiBold),
)

private val shapes = Shapes(
    extraSmall = RoundedCornerShape(4.dp),
    small = RoundedCornerShape(4.dp),
    medium = RoundedCornerShape(6.dp),
    large = RoundedCornerShape(8.dp),
    extraLarge = RoundedCornerShape(12.dp),
)

@Composable
fun CineoTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = colors, typography = typography, shapes = shapes, content = content)
}

@Composable
fun CineoIcon(
    icon: Icon,
    modifier: Modifier = Modifier,
    filled: Boolean = false,
    size: TextUnit = 24.sp,
    tint: Color = Color.Unspecified,
    label: String? = null,
) {
    val useFilled = (filled && icon.filled != null) || icon.regular == null
    val glyph = (if (useFilled) icon.filled else icon.regular) ?: return
    val family = if (useFilled) TablerFilled else TablerRegular
    Text(
        glyph.toString(),
        modifier = modifier.clearAndSetSemantics { label?.let { contentDescription = it } },
        color = tint,
        fontFamily = family,
        fontSize = size,
        lineHeight = size,
        textAlign = TextAlign.Center,
    )
}
