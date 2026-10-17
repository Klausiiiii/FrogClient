package dev.frogclient.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import dev.frogclient.FrogClient;
import dev.frogclient.feature.zoom.ZoomMath;
import dev.frogclient.feature.zoom.ZoomModule;
import net.minecraft.client.Camera;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

@Mixin(Camera.class)
public abstract class CameraMixin {
	@ModifyReturnValue(method = "calculateFov", at = @At("RETURN"))
	private float frogclient$zoomFov(float fov) {
		ZoomModule zoom = FrogClient.modules.get(ZoomModule.class);
		return zoom.isZooming() ? (float) ZoomMath.fov(fov, zoom.currentFactor()) : fov;
	}
}
