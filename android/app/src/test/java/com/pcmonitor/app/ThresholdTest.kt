package com.pcmonitor.app

import com.pcmonitor.app.data.repository.UserPreferencesRepository
import org.junit.Assert.assertEquals
import org.junit.Test

class ThresholdTest {

    @Test
    fun testPreferenceKeyGeneration() {
        val deviceId = "dev_abc123"
        val cpuKey = UserPreferencesRepository.deviceCpuKey(deviceId)
        val gpuKey = UserPreferencesRepository.deviceGpuKey(deviceId)
        val ramKey = UserPreferencesRepository.deviceRamKey(deviceId)
        val diskKey = UserPreferencesRepository.deviceDiskKey(deviceId)
        val hasCustomKey = UserPreferencesRepository.deviceHasCustomKey(deviceId)

        assertEquals("cpu_thresh_dev_abc123", cpuKey.name)
        assertEquals("gpu_thresh_dev_abc123", gpuKey.name)
        assertEquals("ram_thresh_dev_abc123", ramKey.name)
        assertEquals("disk_thresh_dev_abc123", diskKey.name)
        assertEquals("has_custom_thresh_dev_abc123", hasCustomKey.name)
    }
}
