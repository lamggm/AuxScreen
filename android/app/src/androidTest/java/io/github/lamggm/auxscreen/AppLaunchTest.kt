package io.github.lamggm.auxscreen

import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AppLaunchTest {
    @Test
    fun activityLaunchesAndRecreates() {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            scenario.onActivity { assertNotNull(it.window) }
            scenario.recreate()
            scenario.onActivity { assertNotNull(it.window.decorView) }
        }
    }
}
