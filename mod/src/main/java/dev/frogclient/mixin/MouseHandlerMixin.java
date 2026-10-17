package dev.frogclient.mixin;

import dev.frogclient.FrogClient;
import dev.frogclient.feature.zoom.ZoomMath;
import dev.frogclient.feature.zoom.ZoomModule;
import net.minecraft.client.MouseHandler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(MouseHandler.class)
public abstract class MouseHandlerMixin {
	private static final String TURN = "Lnet/minecraft/client/player/LocalPlayer;turn(DD)V";

	@ModifyArg(method = "turnPlayer", at = @At(value = "INVOKE", target = TURN), index = 0)
	private double frogclient$zoomSensitivityX(double xo) {
		return xo * frogclient$sensitivity();
	}

	@ModifyArg(method = "turnPlayer", at = @At(value = "INVOKE", target = TURN), index = 1)
	private double frogclient$zoomSensitivityY(double yo) {
		return yo * frogclient$sensitivity();
	}

	/** While zooming, the wheel changes zoom strength instead of the hotbar slot. */
	@Inject(method = "onScroll", cancellable = true, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/ScrollWheelHandler;onMouseScroll(DD)Lorg/joml/Vector2i;"))
	private void frogclient$zoomScroll(long handle, double xoffset, double yoffset, CallbackInfo ci) {
		if (FrogClient.modules.get(ZoomModule.class).onScroll(yoffset)) ci.cancel();
	}

	private static double frogclient$sensitivity() {
		ZoomModule zoom = FrogClient.modules.get(ZoomModule.class);
		return zoom.isZooming() ? ZoomMath.sensitivityMultiplier(zoom.currentFactor()) : 1.0;
	}
}
