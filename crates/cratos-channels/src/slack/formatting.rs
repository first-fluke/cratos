use super::SlackAdapter;
use crate::message::MessageButton;
use slack_morphism::prelude::*;

impl SlackAdapter {
    /// Build Slack blocks from buttons for interactive messages.
    pub fn build_blocks(text: &str, buttons: &[MessageButton]) -> Vec<SlackBlock> {
        let mut blocks = vec![SlackBlock::Section(SlackSectionBlock::new().with_text(
            SlackBlockText::MarkDown(SlackBlockMarkDownText::new(text.to_string())),
        ))];

        if !buttons.is_empty() {
            let button_elements: Vec<SlackActionBlockElement> = buttons
                .iter()
                .filter_map(|b| {
                    b.callback_data.as_ref().map(|callback_data| {
                        SlackActionBlockElement::Button(SlackBlockButtonElement::new(
                            callback_data.clone().into(),
                            SlackBlockPlainTextOnly::from(b.text.clone()),
                        ))
                    })
                })
                .collect();

            if !button_elements.is_empty() {
                blocks.push(SlackBlock::Actions(SlackActionsBlock::new(button_elements)));
            }
        }

        blocks
    }
}
