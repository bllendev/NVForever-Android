"""The Android fork's values that two languages both have to agree on (docs/fork.md).

Each of these is a folder or file name the Java launcher and the Rust engine or launcher
reach for independently. They cannot be shared at build time, so they are checked here: a
rename on one side that is not matched on the other would send the engine to an empty home,
or the launcher to a game folder nobody chose.
"""
import importlib.util
from pathlib import Path
import re
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "build.py"
spec = importlib.util.spec_from_file_location("android_build", SOURCE)
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)

ROOT = build.ROOT


def text(relative):
    return (ROOT / relative).read_text(encoding="utf-8")


def java_constant(source, name):
    return re.search(rf'String {name} = "([^"]*)"', source).group(1)


class ForkConstantTests(unittest.TestCase):
    def setUp(self):
        self.java = text("android/app/src/main/java/org/nvforever/android/OdinFork.java")

    def test_the_home_folder_is_the_same_on_both_sides(self):
        rust = re.search(r'pub const DIR: &str = "([^"]*)"', text("engine/nvforever/src/odin/home.rs")).group(1)
        self.assertEqual(java_constant(self.java, "HOME"), rust)
        # Upstream's name would look to every existing install like lost saves.
        self.assertNotEqual(rust, "home")

    def test_the_engine_joins_that_name_onto_the_files_directory(self):
        self.assertIn('set_var("NVFOREVER_HOME", files.join(crate::odin::home::DIR))',
                      text("engine/nvforever/src/android.rs"))

    def test_the_data_path_file_and_default_folder_are_the_same_on_both_sides(self):
        launcher = text("crates/nvf-launcher/src/odin.rs")
        for java, rust in (("DATA_PATH", "DATA_PATH_FILE"), ("SHARED_DATA", "SHARED_DATA")):
            value = re.search(rf'pub const {rust}: &str = "([^"]*)"', launcher).group(1)
            self.assertEqual(java_constant(self.java, java), value)

    def test_the_application_id_is_one_gradle_property(self):
        properties = text("android/gradle.properties")
        self.assertIn("nvfApplicationId=org.nvforever.android.odin", properties)
        gradle = text("android/app/build.gradle")
        self.assertIn("providers.gradleProperty('nvfApplicationId')", gradle)
        self.assertIn("applicationId resolvedApplicationId", gradle)
        # The namespace and therefore the Java package stay upstream's.
        self.assertIn("namespace 'org.nvforever.android'", gradle)

    def test_the_debug_variant_installs_beside_the_release_one(self):
        self.assertIn("applicationIdSuffix '.debug'", text("android/app/build.gradle"))


if __name__ == "__main__":
    unittest.main()
