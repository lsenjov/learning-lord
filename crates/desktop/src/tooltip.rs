use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Tooltip(pub String);
#[derive(Component)]
pub(crate) struct Popup;

pub(crate) fn update(
    mut commands: Commands,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    tips: Query<(Entity, &Tooltip, &Interaction, &InheritedVisibility)>,
    (parents, roots): (Query<&ChildOf>, Query<&crate::floating_ui::FloatingWindow>),
    capture: Option<Res<crate::floating_ui::InputCapture>>,
    mut popup: Query<(Entity, &mut Node, &mut Visibility, &Children), With<Popup>>,
    mut labels: Query<&mut Text>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let hovered = tips.iter().find(|(entity, tip, interaction, visible)| {
        !tip.0.is_empty()
            && **interaction == Interaction::Hovered
            && visible.get()
            && capture.as_ref().is_none_or(|capture| {
                crate::navigation::window_kind(*entity, &parents, &roots) == capture.hovered
            })
    });
    let Some((_, tip, _, _)) = hovered else {
        for (_, _, mut visibility, _) in &mut popup {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let width = (window.width() - 16.0).clamp(1.0, 390.0);
    let left = (cursor.x + 12.0).min(window.width() - width - 8.0).max(0.0);
    let top = if cursor.y > window.height() * 0.65 {
        (cursor.y - 150.0).max(0.0)
    } else {
        cursor.y + 18.0
    };
    if let Ok((_, mut node, mut visibility, children)) = popup.single_mut() {
        node.left = px(left);
        node.top = px(top);
        node.width = px(width);
        *visibility = Visibility::Visible;
        for child in children {
            if let Ok(mut text) = labels.get_mut(*child) {
                text.0.clone_from(&tip.0);
            }
        }
    } else {
        commands
            .spawn((
                Popup,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(top),
                    width: px(width),
                    padding: UiRect::all(px(8)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.035, 0.055, 0.07)),
                GlobalZIndex(i32::MAX),
                bevy::ui::FocusPolicy::Pass,
            ))
            .with_children(|parent| {
                parent.spawn(crate::text(&tip.0, 13.0, crate::TEXT));
            });
    }
}
