# `eas.json`'s build profiles. The file itself is generated — `base` gets the
# Rust version, the targets and the engine revision an EAS container installs
# — so this is the part that is this app's: which app each profile builds.
# `APP_VARIANT` is what `app.config.ts` reads to decide between
# `dev.harken.koon.us` and `harken.koon.us`.
{
  perSystem.mobile.eas.profiles = {
    base.android.image = "latest";
    development = {
      extends = "base";
      developmentClient = true;
      distribution = "internal";
      android.buildType = "apk";
      env.APP_VARIANT = "development";
    };
    preview = {
      extends = "base";
      distribution = "internal";
      android.buildType = "apk";
      env.APP_VARIANT = "production";
    };
    production = {
      extends = "base";
      autoIncrement = true;
      android.buildType = "app-bundle";
      env.APP_VARIANT = "production";
    };
  };
}
