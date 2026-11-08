package io.github.mdmrk.cineo.ui

import kotlin.math.abs

private const val SETTLE_S = 1.0
private const val TIMEOUT_MS = 2000L

internal data class SeekHold(
    val drag: Double? = null,
    val sent: Double? = null,
    val sentAtMs: Long = 0,
    val busySeen: Boolean = false,
) {
    val value: Double? get() = drag ?: sent

    fun dragTo(target: Double) = copy(drag = target)

    fun commit(target: Double, nowMs: Long) = SeekHold(sent = target, sentAtMs = nowMs)

    fun commitBy(delta: Double, position: Double, duration: Double, nowMs: Long): SeekHold =
        if (duration > 0.0) commit(((value ?: position) + delta).coerceIn(0.0, duration), nowMs) else this

    fun settle(actual: Double, busy: Boolean, nowMs: Long): SeekHold {
        val target = sent ?: return this
        val finished = busySeen && !busy
        val done = drag == null && (abs(actual - target) <= SETTLE_S || finished || nowMs - sentAtMs > TIMEOUT_MS)
        return if (done) copy(sent = null, busySeen = false) else copy(busySeen = busy)
    }
}
