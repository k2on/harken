/**
 * Which device is making the sound, and everywhere it could be.
 *
 * A sheet rather than a menu, for the reason the playlist sheet is one: the
 * answer is a list of unknown length, and a phone has no room for a dropdown
 * that might hold six devices. It is the same shape as that sheet on purpose
 * — the backdrop closes it, the grabber says it can be dragged, and the last
 * row is the other thing you might want — because two sheets that behave
 * differently are two things to learn.
 *
 * **A device that cannot be heard is drawn and not selectable.** The desktop
 * build has no audio device, so it is a remote control and can never be the
 * output — but it is *in* the session and controlling it, and hiding it would
 * answer "where is my laptop" with silence. "No audio device" is a different
 * answer from "not here", and the list should be able to say which.
 */

import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeIn, FadeOut, SlideInDown } from 'react-native-reanimated';

import type { Device } from '@/listening';
import { FONT, radius, space, type Theme } from '@/theme';
import { Icon } from './icon';

export function Devices({
  devices,
  output,
  me,
  theme,
  bottom,
  onPick,
  onClose,
}: {
  devices: Device[];
  /** The id of the one making the sound, or null. */
  output: string | null;
  /** This device's id, so the list can say which row is you. */
  me: string;
  theme: Theme;
  bottom: number;
  onPick: (to: string | null) => void;
  onClose: () => void;
}) {
  const s = styles(theme);
  return (
    <Animated.View style={s.scrim} entering={FadeIn.duration(140)} exiting={FadeOut.duration(120)}>
      <Pressable style={s.backdrop} onPress={onClose} accessibilityLabel="close" />

      <Animated.View style={[s.sheet, { paddingBottom: bottom + space.lg }]} entering={SlideInDown}>
        <View style={s.grabber} />
        <Text style={s.title}>Playing on</Text>
        <Text style={s.sub} numberOfLines={1}>
          one session, however many devices are watching it
        </Text>

        <ScrollView style={s.list} contentContainerStyle={s.listInner}>
          {devices.length === 0 ? (
            <Text style={s.empty}>
              No devices — this peer has no listening session. Link up to get one.
            </Text>
          ) : (
            devices.map((device) => {
              const here = device.id === output;
              const mine = device.id === me;
              return (
                <Pressable
                  key={device.id}
                  disabled={!device.audible}
                  onPress={() => {
                    onPick(device.id);
                    onClose();
                  }}
                  style={({ pressed }) => [s.row, pressed && s.rowPressed]}
                  accessibilityRole="button"
                  accessibilityState={{ selected: here, disabled: !device.audible }}
                >
                  <Icon
                    name={mine ? 'phone' : 'laptop'}
                    size={21}
                    tint={here ? theme.accent : device.audible ? theme.dim : theme.faint}
                  />
                  <View style={s.rowText}>
                    <Text
                      style={[s.rowName, here && s.rowNameOn, !device.audible && s.rowOff]}
                      numberOfLines={1}
                    >
                      {device.name}
                      {mine ? ' (this one)' : ''}
                    </Text>
                    {device.audible ? null : (
                      <Text style={s.rowWhy}>no audio device — a remote control</Text>
                    )}
                  </View>
                  {here ? <Icon name="playing" size={18} tint={theme.accent} /> : null}
                </Pressable>
              );
            })
          )}

          {/* The last row, the way "New playlist" is the last row of the other
              sheet: the other thing you might want, reachable without a second
              control to find. */}
          <Pressable
            onPress={() => {
              onPick(null);
              onClose();
            }}
            style={({ pressed }) => [s.row, pressed && s.rowPressed]}
            accessibilityRole="button"
          >
            <Icon name="stop" size={21} tint={theme.dim} />
            <Text style={[s.rowName, s.rowText]}>Stop everywhere</Text>
          </Pressable>
        </ScrollView>
      </Animated.View>
    </Animated.View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    // Written out rather than spread from `absoluteFill`: these typings expose
    // it as a registered style and not as an object, so it cannot be spread
    // into one. The same four edges either way.
    scrim: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, justifyContent: 'flex-end' },
    backdrop: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: t.scrim },
    sheet: {
      backgroundColor: t.raised,
      borderTopLeftRadius: radius.xl,
      borderTopRightRadius: radius.xl,
      paddingHorizontal: space.lg,
      paddingTop: space.sm,
      borderTopWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
      maxHeight: '72%',
    },
    grabber: {
      alignSelf: 'center',
      width: 38,
      height: 4,
      borderRadius: radius.pill,
      backgroundColor: t.border,
      marginBottom: space.md,
    },
    title: { fontFamily: FONT, fontSize: 16, fontWeight: '700', color: t.text },
    sub: { fontFamily: FONT, fontSize: 12.5, color: t.dim, marginTop: 2 },
    list: { marginTop: space.md },
    listInner: { paddingBottom: space.sm },
    empty: { color: t.faint, fontFamily: FONT, fontSize: 13, paddingVertical: space.md },
    row: { flexDirection: 'row', alignItems: 'center', gap: space.md, paddingVertical: space.md },
    rowPressed: { backgroundColor: t.cardHigh, borderRadius: radius.md },
    rowText: { flex: 1, gap: 1 },
    rowName: { fontFamily: FONT, fontSize: 15, color: t.text },
    rowNameOn: { color: t.accent, fontWeight: '600' },
    rowOff: { color: t.faint },
    rowWhy: { fontFamily: FONT, fontSize: 11.5, color: t.faint },
  });
