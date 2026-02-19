use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A2UI 서버 → 클라이언트 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum A2uiServerMessage {
    /// UI 컴포넌트 렌더링
    Render {
        component_id: Uuid,
        component_type: A2uiComponentType,
        props: serde_json::Value,
        slot: Option<String>,
    },
    /// 컴포넌트 업데이트
    Update {
        component_id: Uuid,
        props: serde_json::Value,
    },
    /// 컴포넌트 제거
    Remove { component_id: Uuid },
    /// 네비게이션
    Navigate {
        url: String,
        options: NavigateOptions,
    },
    /// 스크린샷 요청
    Snapshot {
        request_id: Uuid,
        format: SnapshotFormat,
    },
    /// 토스트/알림
    Notify {
        message: String,
        level: NotifyLevel,
        duration_ms: Option<u32>,
    },
    /// 모달 표시
    ShowModal {
        modal_id: Uuid,
        title: String,
        content: ModalContent,
        actions: Vec<ModalAction>,
    },
    /// 모달 닫기
    CloseModal { modal_id: Uuid },
}

/// A2UI 클라이언트 → 서버 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum A2uiClientMessage {
    /// 컴포넌트 이벤트 (클릭, 입력 등)
    Event {
        component_id: Uuid,
        event_type: String,
        payload: serde_json::Value,
    },
    /// 스크린샷 응답
    SnapshotResult { request_id: Uuid, data: String },
    /// 모달 액션 응답
    ModalAction { modal_id: Uuid, action_id: String },
    /// 스티어링 (직접 제어)
    Steer {
        /// "abort", "skip", "user_text"
        action: String,
        payload: Option<serde_json::Value>,
    },
    /// 연결 상태
    Ready,
    /// 에러
    Error { code: String, message: String },
}

/// 컴포넌트 타입 (허용 목록)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum A2uiComponentType {
    TextInput,
    TextArea,
    Select,
    Checkbox,
    Radio,
    Slider,
    DatePicker,
    TimePicker,
    FilePicker,
    ColorPicker,
    Text,
    Markdown,
    Code,
    Image,
    Video,
    Audio,
    Iframe,
    Card,
    Modal,
    Drawer,
    Tabs,
    Accordion,
    Divider,
    Spacer,
    Grid,
    Flex,
    Chart,
    Table,
    Progress,
    Spinner,
    Badge,
    Button,
    Link,
    Form,
    Menu,
    Dropdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateOptions {
    pub target: NavigateTarget,
    pub sandbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigateTarget {
    Replace,
    NewTab,
    Iframe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotFormat {
    Png,
    Jpeg,
    Webp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifyLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalContent {
    // TBD: 구조 정의 필요, 임시로 JSON
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalAction {
    pub id: String,
    pub label: String,
    pub variant: String, // primary, secondary, danger
}
