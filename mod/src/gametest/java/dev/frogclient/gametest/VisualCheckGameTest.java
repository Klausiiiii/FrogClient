package dev.frogclient.gametest;

import dev.frogclient.FrogClient;
import dev.frogclient.FrogKeys;
import dev.frogclient.config.GuiState;
import dev.frogclient.feature.FullbrightModule;
import dev.frogclient.feature.zoom.ZoomModule;
import dev.frogclient.gui.ClientScreen;
import dev.frogclient.hud.HudEditorScreen;
import net.fabricmc.fabric.api.client.gametest.v1.FabricClientGameTest;
import net.fabricmc.fabric.api.client.gametest.v1.context.ClientGameTestContext;
import net.fabricmc.fabric.api.client.gametest.v1.context.TestSingleplayerContext;

/**
 * Drives the real client through every feature and saves screenshots for visual review.
 * Run with: gradlew runClientGameTest
 */
public final class VisualCheckGameTest implements FabricClientGameTest {
	@Override
	public void runTest(ClientGameTestContext context) {
		try (TestSingleplayerContext world = context.worldBuilder().create()) {
			world.getConnection().waitForChunksDownload();
			world.getServer().runCommand("time set midnight");
			world.getServer().runCommand("weather clear");
			context.waitTicks(20);

			shot(context, "01-hud-fps");

			context.runOnClient(mc -> FrogClient.modules.get(FullbrightModule.class).setEnabled(false));
			context.waitTicks(5);
			shot(context, "02-night-fullbright-off");
			context.runOnClient(mc -> FrogClient.modules.get(FullbrightModule.class).setEnabled(true));
			context.waitTicks(5);
			shot(context, "03-night-fullbright-on");
			world.getServer().runCommand("time set noon");
			context.waitTicks(5);
			shot(context, "03b-day-fullbright-on");
			world.getServer().runCommand("time set midnight");
			context.waitTicks(5);
			double gamma = context.computeOnClient(mc -> mc.options.gamma().get());
			check(gamma == 0.5, "options gamma changed to " + gamma);

			context.getInput().holdKey(FrogKeys.ZOOM);
			context.waitTicks(5);
			check(context.computeOnClient(mc -> FrogClient.modules.get(ZoomModule.class).isZooming()), "zoom not active while key held");
			shot(context, "04-zoom");
			context.getInput().releaseKey(FrogKeys.ZOOM);
			context.waitTicks(2);
			check(!context.computeOnClient(mc -> FrogClient.modules.get(ZoomModule.class).isZooming()), "zoom still active after release");

			context.getInput().pressKey(FrogKeys.OPEN_GUI);
			context.waitForScreen(ClientScreen.class);
			context.runOnClient(mc -> FrogClient.gui.setLayout(GuiState.Layout.PANELS));
			context.setScreen(ClientScreen::new);
			context.waitTicks(2);
			shot(context, "05-gui-panels");

			double scale = context.computeOnClient(mc -> (double) mc.getWindow().getGuiScale());
			context.getInput().setCursorPos((10 + 30) * scale, (26 + 16 + 14 + 7) * scale);
			context.getInput().pressMouse(1);
			context.waitTicks(2);
			shot(context, "06-gui-panels-zoom-expanded");

			context.clickScreenButton("frogclient.gui.layout.panels");
			context.waitTicks(2);
			shot(context, "07-gui-window");

			context.setScreen(() -> new HudEditorScreen(null));
			context.waitTicks(2);
			shot(context, "08-hud-editor");
			context.setScreen(() -> null);
		}
	}

	private static void shot(ClientGameTestContext context, String name) {
		FrogClient.LOG.info("[visual-check] screenshot {}", context.takeScreenshot(name));
	}

	private static void check(boolean ok, String message) {
		if (!ok) throw new AssertionError(message);
	}
}
