package dev.frogclient.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import dev.frogclient.FrogClient;
import dev.frogclient.feature.FullbrightModule;
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import net.minecraft.client.renderer.state.LightmapRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(LightmapRenderStateExtractor.class)
public abstract class LightmapMixin {
	/** The first Double#floatValue in extract() reads the gamma option. */
	@ModifyExpressionValue(
			method = "extract",
			at = @At(value = "INVOKE", target = "Ljava/lang/Double;floatValue()F", ordinal = 0))
	private float frogclient$fullbright(float gamma) {
		return (float) FrogClient.modules.get(FullbrightModule.class).effectiveGamma(gamma);
	}

	@Inject(method = "extract", at = @At("TAIL"))
	private void frogclient$fullbrightAmbient(LightmapRenderState renderState, float partialTicks, CallbackInfo ci) {
		if (renderState.needsUpdate && FrogClient.modules.get(FullbrightModule.class).forcesFullAmbient()) {
			renderState.ambientColor = LightmapRenderStateExtractor.WHITE;
		}
	}
}
