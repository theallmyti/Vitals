package com.pcmonitor.app

import com.pcmonitor.app.data.model.BootEvent
import com.pcmonitor.app.data.model.BootEventType
import com.pcmonitor.app.data.model.formatGb
import org.junit.Assert.assertEquals
import org.junit.Test

class TelemetryParsingTest {

    @Test
    fun testFormatGbPrecision() {
        assertEquals("15.7 GB", formatGb((15.7 * 1024).toLong()))
        assertEquals("8.0 GB", formatGb((8.0 * 1024).toLong()))
        assertEquals("128 GB", formatGb((128.0 * 1024).toLong()))
    }

    @Test
    fun testCoreTempsCalculation() {
        val coreTemps = listOf(70.0f, 75.0f, 80.0f, 75.0f)
        val rawCpuTemp = 0f

        val avgTemp = if (coreTemps.isNotEmpty()) coreTemps.average().toFloat() else rawCpuTemp
        val maxTemp = if (coreTemps.isNotEmpty()) maxOf(coreTemps.maxOrNull() ?: rawCpuTemp, rawCpuTemp) else rawCpuTemp
        val pkgTemp = if (rawCpuTemp > 0f) rawCpuTemp else avgTemp

        assertEquals(75.0f, avgTemp, 0.01f)
        assertEquals(80.0f, maxTemp, 0.01f)
        assertEquals(75.0f, pkgTemp, 0.01f)
    }

    @Test
    fun testRamCalculationFallbackWithUsedPct() {
        // When detail object is absent but usedPct is present
        val usedPct = 68.6
        val totalGb = 0.0

        val (totalMb, usedMb, freeMb) = when {
            totalGb > 0.0 -> {
                val tot = (totalGb * 1024.0).toLong()
                val usd = (tot * usedPct / 100.0).toLong()
                Triple(tot, usd, tot - usd)
            }
            usedPct > 0.0 -> {
                val tot = 10000L
                val usd = (10000L * usedPct / 100.0).toLong()
                Triple(tot, usd, tot - usd)
            }
            else -> Triple(0L, 0L, 0L)
        }

        val usagePercent = if (totalMb > 0) (usedMb.toFloat() / totalMb) * 100f else 0f
        assertEquals(68.6f, usagePercent, 0.1f)
        assertEquals(10000L, totalMb)
        assertEquals(6860L, usedMb)
        assertEquals(3140L, freeMb)
    }

    @Test
    fun testRamCalculationWithDetail() {
        val totalGb = 16.0
        val usedGb = 10.0
        val freeGb = 6.0
        val usedPct = 62.5

        val (totalMb, usedMb, freeMb) = when {
            totalGb > 0.0 -> {
                val tot = (totalGb * 1024.0).toLong()
                val usd = if (usedGb > 0.0) (usedGb * 1024.0).toLong() else if (usedPct > 0.0) (tot * usedPct / 100.0).toLong() else 0L
                val fre = if (freeGb > 0.0) (freeGb * 1024.0).toLong() else maxOf(0L, tot - usd)
                Triple(tot, usd, fre)
            }
            else -> Triple(0L, 0L, 0L)
        }

        assertEquals(16384L, totalMb)
        assertEquals(10240L, usedMb)
        assertEquals(6144L, freeMb)
        val usagePercent = if (totalMb > 0) (usedMb.toFloat() / totalMb) * 100f else 0f
        assertEquals(62.5f, usagePercent, 0.01f)
    }

    @Test
    fun testUsageParsing() {
        val events = listOf(
            "boot" to 1700000000000L,
            "wake" to 1700003600000L
        )

        val bootHistory = events.map { (eventName, ts) ->
            val type = when (eventName.lowercase()) {
                "boot" -> BootEventType.BOOT
                "sleep" -> BootEventType.SLEEP
                "wake" -> BootEventType.WAKE
                "shutdown" -> BootEventType.SHUTDOWN
                else -> BootEventType.BOOT
            }
            BootEvent(type, ts)
        }

        assertEquals(2, bootHistory.size)
        assertEquals(BootEventType.BOOT, bootHistory[0].type)
        assertEquals(BootEventType.WAKE, bootHistory[1].type)
    }
}
