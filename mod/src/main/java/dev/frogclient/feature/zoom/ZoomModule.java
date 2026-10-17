package dev.frogclient.feature.zoom;

import dev.frogclient.module.Category;
import dev.frogclient.module.Module;
import dev.frogclient.setting.BoolSetting;
import dev.frogclient.setting.SliderSetting;
import net.minecraft.client.Minecraft;

import java.util.function.BooleanSupplier;
import java.util.function.Consumer;

/** Hold-to-zoom. Smooth camera is borrowed during the zoom and restored exactly afterwards. */
public final class ZoomModule extends Module {
	public final SliderSetting factor = add(new SliderSetting("factor", "frogclient.setting.zoom.factor", 2, 10, 0.5, 4));
	public final BoolSetting scroll = add(new BoolSetting("scroll", "frogclient.setting.zoom.scroll", true));
	public final BoolSetting smooth = add(new BoolSetting("smooth", "frogclient.setting.zoom.smooth", false));

	private final BooleanSupplier smoothCameraGetter;
	private final Consumer<Boolean> smoothCameraSetter;

	private boolean zooming;
	private double currentFactor;
	private boolean smoothBorrowed;
	private boolean smoothBefore;

	public ZoomModule() {
		this(() -> Minecraft.getInstance().options.smoothCamera,
				v -> Minecraft.getInstance().options.smoothCamera = v);
	}

	public ZoomModule(BooleanSupplier smoothCameraGetter, Consumer<Boolean> smoothCameraSetter) {
		super("zoom", Category.RENDER, true);
		this.smoothCameraGetter = smoothCameraGetter;
		this.smoothCameraSetter = smoothCameraSetter;
	}

	public boolean isZooming() {
		return zooming;
	}

	public double currentFactor() {
		return currentFactor;
	}

	public void tick(boolean keyDown, boolean screenOpen, boolean windowFocused) {
		boolean shouldZoom = isEnabled() && keyDown && !screenOpen && windowFocused;
		if (shouldZoom && !zooming) start();
		else if (!shouldZoom && zooming) stop();
	}

	/** @return true if the scroll event was used for zooming and must not reach the hotbar */
	public boolean onScroll(double delta) {
		if (!zooming || !scroll.isOn() || delta == 0) return false;
		currentFactor = ZoomMath.scroll(currentFactor, delta);
		return true;
	}

	private void start() {
		zooming = true;
		currentFactor = factor.value();
		if (smooth.isOn()) {
			smoothBefore = smoothCameraGetter.getAsBoolean();
			smoothBorrowed = true;
			smoothCameraSetter.accept(true);
		}
	}

	private void stop() {
		zooming = false;
		if (smoothBorrowed) {
			smoothCameraSetter.accept(smoothBefore);
			smoothBorrowed = false;
		}
	}

	@Override
	protected void onDisable() {
		if (zooming) stop();
	}
}
