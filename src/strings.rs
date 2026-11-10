//! The interface's words in English and Russian.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Russian,
}

impl Language {
    pub fn other(self) -> Language {
        match self {
            Language::English => Language::Russian,
            Language::Russian => Language::English,
        }
    }
}

macro_rules! strings {
    ($($id:ident: $english:literal, $russian:literal;)*) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub enum Str {
            $($id),*
        }

        const TABLE: &[(&str, &str)] = &[$(($english, $russian)),*];
    };
}

strings! {
    Title: "3D Checkers", "Шашки 3D";
    WhiteToMove: "White to move", "Ход белых";
    BlackToMove: "Black to move", "Ход чёрных";
    MustTake: "Must take", "Нужно бить";
    Thinking: "Computer is thinking…", "Компьютер думает…";
    Draw: "Draw", "Ничья";
    WhiteWins: "White wins", "Белые победили";
    BlackWins: "Black wins", "Чёрные победили";
    NoPieces: "no pieces left", "шашек не осталось";
    NoMoves: "no moves left", "ходов не осталось";
    NoMovesGiveaway: "ran out of moves first", "первыми остались без ходов";
    ByRepetition: "by threefold repetition", "троекратное повторение";
    ByQuietMoves: "kings only, nothing taken", "ходы дамками без взятий";
    ByLoneKings: "a king against a king", "дамка против дамки";
    You: "You", "Вы";
    Computer: "Computer", "Компьютер";
    Player: "Player", "Игрок";
    White: "White", "Белые";
    Black: "Black", "Чёрные";
    Moves: "Moves", "Ходы";
    NoMovesYet: "No moves yet", "Ходов пока нет";
    Undo: "Undo", "Назад";
    Redo: "Redo", "Вперёд";
    Hint: "Hint", "Подсказка";
    NewGame: "New game", "Новая партия";
    Flip: "Flip board", "Развернуть";
    Mode: "Mode", "Режим";
    ModeWhite: "You play White", "Вы за белых";
    ModeBlack: "You play Black", "Вы за чёрных";
    ModeTwoPlayers: "Two players", "Два игрока";
    ModeDemo: "Computer vs computer", "Компьютер сам с собой";
    Level: "Level", "Уровень";
    Level1: "Beginner", "Новичок";
    Level2: "Casual", "Любитель";
    Level3: "Club player", "Разрядник";
    Level4: "Expert", "Сильный";
    Level5: "Master", "Мастер";
    Rules: "Rules", "Правила";
    RulesRussian: "Russian", "Русские";
    RulesEnglish: "English", "Английские";
    RulesGiveaway: "Giveaway", "Поддавки";
    Theme: "Theme", "Тема";
    Sound: "Sound", "Звук";
    On: "On", "Вкл";
    Off: "Off", "Выкл";
    LanguageName: "English", "Русский";
    Footer: "F1 help · Tab panel · F11 full screen", "F1 справка · Tab панель · F11 весь экран";
    Controls: "Controls", "Управление";
    KeyClick: "Click", "Щелчок";
    KeyDrag: "Drag", "Перетаскивание";
    KeyRightDrag: "Right drag", "Правая кнопка";
    KeyWheel: "Wheel", "Колесо мыши";
    HelpClick: "Click a piece, then where it goes", "Щелчок по шашке, затем по полю";
    HelpDrag: "Or drag the piece", "Или перетащите шашку";
    HelpRotate: "Turn the view (or drag beside the pieces)", "Повернуть вид (или тянуть мимо шашек)";
    HelpZoom: "Zoom in and out", "Приблизить или отдалить";
    HelpUndo: "Take back", "Отменить ход";
    HelpRedo: "Replay", "Вернуть ход";
    HelpHint: "Suggest a move", "Подсказать ход";
    HelpNew: "New game", "Новая партия";
    HelpFlip: "Flip the board", "Развернуть доску";
    HelpTopView: "View from above", "Вид сверху";
    HelpLevel: "Computer level", "Уровень компьютера";
    HelpMode: "Who plays whom", "Кто с кем играет";
    HelpRules: "Russian, English, giveaway", "Русские, английские, поддавки";
    HelpTheme: "Next theme", "Следующая тема";
    HelpSound: "Sound on or off", "Звук вкл/выкл";
    HelpLanguage: "English / Russian", "Русский / английский";
    HelpPanel: "Show or hide the panel", "Показать/скрыть панель";
    HelpFullScreen: "Full screen", "Весь экран";
    HelpCopy: "Copy the game (PDN), Shift: the position", "Копировать партию (PDN), Shift: позицию";
    HelpPaste: "Paste a PDN game or position", "Вставить партию или позицию PDN";
    HelpClose: "Close this help", "Закрыть справку";
    RulesNoteRussian: "Men move forward and take both ways. Taking is compulsory. Kings fly any distance. White begins.",
        "Простые ходят вперёд, бьют вперёд и назад. Бить обязательно. Дамка ходит на любое поле. Начинают белые.";
    RulesNoteEnglish: "Men move and take forward only. Taking is compulsory. Kings move one square. Black begins.",
        "Простые ходят и бьют только вперёд. Бить обязательно. Дамка ходит на одно поле. Начинают чёрные.";
    RulesNoteGiveaway: "Russian rules, but the aim is to lose all your pieces or be left without a move.",
        "Русские правила, но цель — отдать все шашки или остаться без ходов.";
    PdnCopied: "Game copied as PDN", "Партия скопирована (PDN)";
    FenCopied: "Position copied", "Позиция скопирована";
    GameLoaded: "Game loaded", "Партия загружена";
    NothingToPaste: "The clipboard has no game or position", "В буфере нет партии или позиции";
    HintToast: "Hint: ", "Подсказка: ";
    NothingToUndo: "No moves to take back", "Нечего отменять";
    TakingIsCompulsory: "Taking is compulsory", "Бить обязательно";
    ChooseRoute: "Several captures end here: click the squares on the way", "Сюда ведут разные взятия: укажите поля по пути";
    NewRules: "New game: ", "Новая партия: ";
}

impl Str {
    /// The name of a computer level from 1 to 5.
    pub fn level(level: i32) -> Str {
        [Str::Level1, Str::Level2, Str::Level3, Str::Level4, Str::Level5][(level.clamp(1, 5) - 1) as usize]
    }
}

pub fn tr(id: Str, language: Language) -> &'static str {
    let (english, russian) = TABLE[id as usize];
    match language {
        Language::English => english,
        Language::Russian => russian,
    }
}
