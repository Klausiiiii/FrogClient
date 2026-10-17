package dev.frogclient.feature.zoom;

import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicBoolean;

import static org.junit.jupiter.api.Assertions.*;

class ZoomModuleTest {
	private final AtomicBoolean smoothCamera = new AtomicBoolean(false);
	private final ZoomModule zoom = new ZoomModule(smoothCamera::get, smoothCamera::set);

	@Test
	void zoomsOnlyWhileKeyHeldWithoutScreen() {
		zoom.tick(true, false, true);
		assertTrue(zoom.isZooming());
		assertEquals(4.0, zoom.currentFactor());
		zoom.tick(false, false, true);
		assertFalse(zoom.isZooming());
		zoom.tick(true, true, true);
		assertFalse(zoom.isZooming());
	}

	@Test
	void openingScreenWhileHeldEndsZoomAndRestoresSmooth() {
		zoom.smooth.set(true);
		zoom.tick(true, false, true);
		assertTrue(smoothCamera.get());
		zoom.tick(true, true, true);
		assertFalse(zoom.isZooming());
		assertFalse(smoothCamera.get());
	}

	@Test
	void smoothCameraKeepsUserValueWhenAlreadyOn() {
		smoothCamera.set(true);
		zoom.smooth.set(true);
		zoom.tick(true, false, true);
		zoom.tick(false, false, true);
		assertTrue(smoothCamera.get());
	}

	@Test
	void smoothOffLeavesCameraUntouched() {
		zoom.tick(true, false, true);
		assertFalse(smoothCamera.get());
	}

	@Test
	void focusLossEndsZoom() {
		zoom.tick(true, false, true);
		zoom.tick(true, false, false);
		assertFalse(zoom.isZooming());
	}

	@Test
	void scrollChangesFactorOnlyWhileZoomingAndResetsAfter() {
		assertFalse(zoom.onScroll(1));
		zoom.tick(true, false, true);
		assertTrue(zoom.onScroll(1));
		assertEquals(5.0, zoom.currentFactor(), 1e-9);
		zoom.tick(false, false, true);
		zoom.tick(true, false, true);
		assertEquals(4.0, zoom.currentFactor(), 1e-9);
	}

	@Test
	void scrollNotConsumedWhenScrollSettingOff() {
		zoom.scroll.set(false);
		zoom.tick(true, false, true);
		assertFalse(zoom.onScroll(1));
		assertEquals(4.0, zoom.currentFactor(), 1e-9);
	}

	@Test
	void disabledModuleNeverZooms() {
		zoom.setEnabled(false);
		zoom.tick(true, false, true);
		assertFalse(zoom.isZooming());
	}

	@Test
	void disablingWhileZoomingEndsZoom() {
		zoom.smooth.set(true);
		zoom.tick(true, false, true);
		zoom.setEnabled(false);
		assertFalse(zoom.isZooming());
		assertFalse(smoothCamera.get());
	}
}
