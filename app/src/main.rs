use leptos::prelude::*;
use model::{Catalog, Description, Lang, Media, MediaKind, QuizMode, Species};
use quiz_core::{generate, rng::Rng, Difficulty, Question};
mod progress;

use progress::Progress;
use std::sync::Arc;

const QUIZ_LEN: usize = 10;
const N_OPTIONS: usize = 4;

// Mode glyphs, drawn in currentColor so they follow the tile's text color.
const GLYPH_PHOTO: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="6" width="18" height="14" rx="3"/><circle cx="12" cy="13" r="3.5"/><path d="M9 6l1.5-2h3L15 6"/></svg>"#;
const GLYPH_SOUND: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/></svg>"#;
const GLYPH_FEATHER: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 4C11 4 6 10 6 18"/><path d="M20 4c0 8-5 13-13 13"/><path d="M4 20l3-3"/></svg>"#;
const GLYPH_TEXT: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 6h14M5 11h14M5 16h9"/></svg>"#;

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Home,
    Regions,
    Quiz,
    Result,
}

#[derive(Clone)]
struct QuizState {
    mode: QuizMode,
    questions: Vec<Question>,
    /// Random pick among a species' media, one per question.
    variants: Vec<usize>,
    idx: usize,
    picked: Option<i64>,
    score: usize,
}

/// Birds that can be asked in `mode` in the chosen regions (empty = all of Europe).
/// They need the matching media or text.
fn pool_for(cat: &Catalog, mode: QuizMode, lang: Lang, regions: &[String]) -> Vec<Species> {
    let has_media = |id: i64, kind: MediaKind| cat.media.iter().any(|m| m.species_id == id && m.kind == kind);
    cat.species
        .iter()
        .filter(|s| {
            regions.is_empty() || cat.species_regions.iter().any(|r| r.species_id == s.id && regions.contains(&r.region))
        })
        .filter(|s| match mode {
            QuizMode::Photo => has_media(s.id, MediaKind::Photo),
            QuizMode::Feather => has_media(s.id, MediaKind::Feather),
            QuizMode::Sound => has_media(s.id, MediaKind::Sound),
            QuizMode::Description => description_for(cat, s.id, lang).is_some(),
        })
        .cloned()
        .collect()
}

fn region_name(cat: &Catalog, code: &str, lang: Lang) -> String {
    cat.regions
        .iter()
        .find(|r| r.code == code)
        .map(|r| if lang == Lang::En { r.name_en.clone() } else { r.name_fr.clone() })
        .unwrap_or_else(|| code.to_string())
}

fn description_for(cat: &Catalog, species_id: i64, lang: Lang) -> Option<&Description> {
    cat.descriptions.iter().find(|d| d.species_id == species_id && d.lang == lang)
}

fn new_quiz(cat: &Catalog, progress: &Progress, mode: QuizMode, lang: Lang, difficulty: Difficulty, regions: &[String]) -> QuizState {
    let pool = pool_for(cat, mode, lang, regions);
    let mut rng = Rng::new(js_sys::Date::now() as u64);
    let mut ids: Vec<i64> = pool.iter().map(|s| s.id).collect();
    // Shuffle first so equally urgent birds come in random order, then most urgent first.
    rng.shuffle(&mut ids);
    ids.sort_by_key(|id| std::cmp::Reverse(progress.priority(mode, *id)));
    ids.truncate(QUIZ_LEN);
    rng.shuffle(&mut ids);
    let questions = generate(&ids, &pool, N_OPTIONS, difficulty, &mut rng);
    let variants = questions.iter().map(|_| rng.below(1000)).collect();
    QuizState { mode, questions, variants, idx: 0, picked: None, score: 0 }
}

fn media_for(cat: &Catalog, species_id: i64, kind: MediaKind, variant: usize) -> Option<&Media> {
    let items: Vec<&Media> = cat.media.iter().filter(|m| m.species_id == species_id && m.kind == kind).collect();
    (!items.is_empty()).then(|| items[variant % items.len()])
}

fn species_name(cat: &Catalog, id: i64, lang: Lang) -> String {
    cat.species.iter().find(|s| s.id == id).map(|s| s.name(lang).to_string()).unwrap_or_default()
}

/// Night Forest player: waveform that fills while playing, play/pause, replay, half speed.
#[component]
fn SoundPlayer(media: Media) -> impl IntoView {
    let audio = NodeRef::<leptos::html::Audio>::new();
    let playing = RwSignal::new(false);
    let fraction = RwSignal::new(0.0_f64);
    let slow = RwSignal::new(false);
    // Play from an in-memory blob: Android's asset handler has no range requests,
    // so streamed Ogg/Opus reports an infinite duration and the waveform never fills.
    let src = RwSignal::new(None::<String>);
    let blob = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    {
        let (path, blob) = (format!("media/{}", media.file), blob.clone());
        leptos::task::spawn_local(async move {
            let url = blob_url(&path).await;
            *blob.lock().unwrap() = url.clone();
            src.try_set(Some(url.unwrap_or(path)));
        });
    }
    on_cleanup(move || {
        if let Some(url) = blob.lock().unwrap().take() {
            let _ = web_sys::Url::revoke_object_url(&url);
        }
    });

    let toggle = move |_| {
        if let Some(a) = audio.get() {
            if playing.get_untracked() {
                let _ = a.pause();
                playing.set(false);
            } else {
                let _ = a.play();
                playing.set(true);
            }
        }
    };
    let replay = move |_| {
        if let Some(a) = audio.get() {
            a.set_current_time(0.0);
            let _ = a.play();
            playing.set(true);
        }
    };
    let half_speed = move |_| {
        slow.update(|s| *s = !*s);
        if let Some(a) = audio.get() {
            a.set_playback_rate(if slow.get_untracked() { 0.5 } else { 1.0 });
        }
    };
    // Clips exported before their waveform was computed get a generic shape.
    let waveform: Vec<u8> = if media.waveform.is_empty() {
        (0..44).map(|i| (30.0 + 60.0 * ((i as f64 * 0.55).sin() * (i as f64 * 0.21).cos()).abs()) as u8).collect()
    } else {
        media.waveform.clone()
    };
    let n = waveform.len().max(1) as f64;
    let bars = waveform.into_iter().enumerate().map(|(i, h)| {
        view! {
            <span style=move || format!(
                "height: {}%; background: {}",
                h,
                if (i as f64) / n < fraction.get() { "var(--accent)" } else { "#2f4738" }
            )></span>
        }
    }).collect_view();

    view! {
        <div class="player">
            <audio
                node_ref=audio
                src=move || src.get()
                preload="auto"
                on:timeupdate=move |_| {
                    if let Some(a) = audio.get() {
                        let d = a.duration();
                        if d.is_finite() && d > 0.0 { fraction.set(a.current_time() / d); }
                    }
                }
                on:ended=move |_| { playing.set(false); fraction.set(0.0); }
            ></audio>
            <div class="wave">{bars}</div>
            <div class="controls">
                <button class="round" aria-label="Half speed" on:click=half_speed style=move || if slow.get() { "color: var(--accent); border-color: var(--accent)" } else { "" }>"0.5×"</button>
                <button class="play" aria-label="Play or pause" on:click=toggle>
                    {move || if playing.get() { "❚❚" } else { "▶" }}
                </button>
                <button class="round" aria-label="Replay" on:click=replay>"↺"</button>
            </div>
            <span class="credit-line">{format!("{} · {} · {}", media.credit.author, media.credit.license, media.credit.source)}</span>
        </div>
    }
}

#[component]
fn App(catalog: Arc<Catalog>) -> impl IntoView {
    let screen = RwSignal::new(Screen::Home);
    let lang = RwSignal::new(Lang::En);
    let quiz = RwSignal::new(None::<QuizState>);
    let progress = RwSignal::new(Progress::load());
    let difficulty = RwSignal::new(match progress::load_pref("difficulty").as_deref() {
        Some("easy") => Difficulty::Easy,
        Some("hard") => Difficulty::Hard,
        _ => Difficulty::Normal,
    });

    let regions = RwSignal::new(
        progress::load_pref("regions")
            .map(|v| v.split(',').filter(|c| !c.is_empty()).map(String::from).collect::<Vec<_>>())
            .unwrap_or_default(),
    );

    let start = {
        let cat = catalog.clone();
        move |mode: QuizMode| {
            quiz.set(Some(new_quiz(&cat, &progress.get_untracked(), mode, lang.get_untracked(), difficulty.get_untracked(), &regions.get_untracked())));
            screen.set(Screen::Quiz);
        }
    };

    // Birds a mode can ask about in the chosen regions; playable once one question can be built.
    let bird_count = {
        let cat = catalog.clone();
        move |mode: QuizMode| pool_for(&cat, mode, lang.get(), &regions.get()).len()
    };
    let playable = {
        let bird_count = bird_count.clone();
        move |mode: QuizMode| bird_count(mode) >= N_OPTIONS
    };
    let total_species = {
        let bird_count = bird_count.clone();
        move || bird_count(QuizMode::Photo)
    };
    let region_label = {
        let cat = catalog.clone();
        move || {
            let l = lang.get();
            let sel = regions.get();
            match sel.as_slice() {
                [] => if l == Lang::En { "All Europe".to_string() } else { "Toute l'Europe".to_string() },
                [one] => region_name(&cat, one, l),
                [first, rest @ ..] => format!("{} +{}", region_name(&cat, first, l), rest.len()),
            }
        }
    };
    let home = move || {
        let start = start.clone();
        view! {
            <main class="screen">
                <div class="topbar">
                    <span class="logo"><img src="logo.svg" alt="" width="28" height="28"/><b>"quizzo"<span>"zoizo"</span></b></span>
                    <div class="top-actions">
                        <button class="pill" on:click=move |_| screen.set(Screen::Regions)>{region_label.clone()}</button>
                        <button class="pill" on:click=move |_| lang.update(|l| *l = if *l == Lang::En { Lang::Fr } else { Lang::En })>
                            {move || if lang.get() == Lang::En { "EN" } else { "FR" }}
                        </button>
                    </div>
                </div>
                <h1 class="hello">{move || if lang.get() == Lang::En { "Which bird is it today?" } else { "Quel oiseau aujourd'hui ?" }}</h1>
                <div class="stats">
                    <div><b>{move || progress.with(|p| p.correct_total())}</b><small>{move || if lang.get() == Lang::En { "correct" } else { "bonnes réponses" }}</small></div>
                    <div><b>{move || progress.with(|p| p.0.len())}</b><small>{move || if lang.get() == Lang::En { "seen" } else { "vus" }}</small></div>
                    <div><b>{total_species.clone()}</b><small>{move || if lang.get() == Lang::En { "in quiz" } else { "au total" }}</small></div>
                </div>
                <span class="mono">{move || if lang.get() == Lang::En { "Difficulty" } else { "Difficulté" }}</span>
                <div class="seg">
                    {[(Difficulty::Easy, "easy", "Easy", "Facile"), (Difficulty::Normal, "normal", "Normal", "Normal"), (Difficulty::Hard, "hard", "Hard", "Difficile")]
                        .into_iter()
                        .map(|(d, key, en, fr)| view! {
                            <button class:on=move || difficulty.get() == d on:click=move |_| { difficulty.set(d); progress::save_pref("difficulty", key); }>
                                {move || if lang.get() == Lang::En { en } else { fr }}
                            </button>
                        })
                        .collect_view()}
                </div>
                <span class="mono">{move || if lang.get() == Lang::En { "Modes" } else { "Modes de jeu" }}</span>
                <div class="mode-grid">
                    {[
                        (QuizMode::Photo, "Photo", "Photo", GLYPH_PHOTO),
                        (QuizMode::Sound, "Song", "Chant", GLYPH_SOUND),
                        (QuizMode::Feather, "Feather", "Plume", GLYPH_FEATHER),
                        (QuizMode::Description, "Field notes", "Description", GLYPH_TEXT),
                    ]
                        .into_iter()
                        .map(|(mode, en, fr, glyph)| {
                            let (p, count, start) = (playable.clone(), bird_count.clone(), start.clone());
                            view! {
                                <button class="mode" class:hero=mode == QuizMode::Photo disabled={ let p = p.clone(); move || !p(mode) } on:click=move |_| start(mode)>
                                    <span class="glyph" inner_html=glyph></span>
                                    <span>
                                    <b>{move || if lang.get() == Lang::En { en } else { fr }}</b>
                                    <small>{move || {
                                        let n = count(mode);
                                        match (n >= N_OPTIONS, lang.get() == Lang::En) {
                                            (true, true) => format!("{n} birds"),
                                            (true, false) => format!("{n} oiseaux"),
                                            (false, true) => "not enough birds".to_string(),
                                            (false, false) => "pas assez d'oiseaux".to_string(),
                                        }
                                    }}</small>
                                    </span>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </main>
        }
    };

    let regions_view = {
        let cat = catalog.clone();
        move || {
            let cat = cat.clone();
            let rows = cat.regions.iter().map(|r| {
                let code = r.code.clone();
                let n = cat.species_regions.iter().filter(|sr| sr.region == r.code).count();
                let (code_click, code_class) = (code.clone(), code.clone());
                let name = { let cat = cat.clone(); let code = code.clone(); move || region_name(&cat, &code, lang.get()) };
                view! {
                    <button class="region" class:on=move || regions.with(|s| s.contains(&code_class)) on:click=move |_| {
                        regions.update(|s| if let Some(i) = s.iter().position(|c| *c == code_click) { s.remove(i); } else { s.push(code_click.clone()); });
                        progress::save_pref("regions", &regions.get_untracked().join(","));
                    }>
                        <span>{name}</span>
                        <small>{n}</small>
                    </button>
                }
            }).collect_view();
            view! {
                <main class="screen">
                    <div class="topbar">
                        <button class="close" aria-label="Back" on:click=move |_| screen.set(Screen::Home)>"←"</button>
                        <span class="mono">{move || if lang.get() == Lang::En { "Regions" } else { "Régions" }}</span>
                        <span class="mono"></span>
                    </div>
                    <p class="note">{move || if lang.get() == Lang::En {
                        "Pick one or more regions. The quiz only asks about birds seen there. Select none for all of Europe."
                    } else {
                        "Choisissez une ou plusieurs régions. Le quiz ne pose des questions que sur les oiseaux qu'on y observe. Aucune sélection : toute l'Europe."
                    }}</p>
                    <button class="region" class:on=move || regions.with(|s| s.is_empty()) on:click=move |_| {
                        regions.set(vec![]);
                        progress::save_pref("regions", "");
                    }>
                        <span>{move || if lang.get() == Lang::En { "All Europe" } else { "Toute l'Europe" }}</span>
                    </button>
                    <div class="region-list">{rows}</div>
                    <button class="primary" on:click=move |_| screen.set(Screen::Home)>
                        {move || if lang.get() == Lang::En { "Done" } else { "Terminé" }}
                    </button>
                </main>
            }
        }
    };

    let quiz_view = {
        let cat = catalog.clone();
        move || {
            let cat = cat.clone();
            let cat2 = cat.clone();
            view! {
                {move || {
                    let Some(q) = quiz.get() else { return ().into_any() };
                    let Some(question) = q.questions.get(q.idx).cloned() else { return ().into_any() };
                    let mode = q.mode;
                    let variant = q.variants.get(q.idx).copied().unwrap_or(0);
                    let image_kind = if mode == QuizMode::Feather { MediaKind::Feather } else { MediaKind::Photo };
                    let photo = matches!(mode, QuizMode::Photo | QuizMode::Feather)
                        .then(|| media_for(&cat, question.target, image_kind, variant).cloned())
                        .flatten();
                    let sound = (mode == QuizMode::Sound).then(|| media_for(&cat, question.target, MediaKind::Sound, variant).cloned()).flatten();
                    let riddle = (mode == QuizMode::Description)
                        .then(|| description_for(&cat, question.target, lang.get_untracked()).cloned())
                        .flatten();
                    let total = q.questions.len();
                    let answered = q.picked.is_some();
                    let options = question.options.clone();
                    let cat = cat2.clone();
                    view! {
                        <main class="screen">
                            <div class="topbar">
                                <button class="close" aria-label="Quit" on:click=move |_| screen.set(Screen::Home)>"✕"</button>
                                <span class="mono">{format!("{:02} / {:02}", q.idx + 1, total)}</span>
                                <span class="mono">{format!("{} pts", q.score)}</span>
                            </div>
                            <div class="bar"><div style=format!("width: {}%", (q.idx + answered as usize) * 100 / total)></div></div>
                            {sound.map(|m| view! { <SoundPlayer media=m/> })}
                            {riddle.map(|d| view! {
                                <div class="riddle">
                                    <span class="mono">{if lang.get_untracked() == Lang::En { "Who am I?" } else { "Qui suis-je ?" }}</span>
                                    <p>{d.text}</p>
                                    <span class="credit-line">"Adapted from Wikipedia · CC BY-SA 4.0"</span>
                                </div>
                            })}
                            {photo.map(|m| view! {
                                <span class="mono">{match (mode, lang.get_untracked() == Lang::En) {
                                    (QuizMode::Feather, true) => "Whose feather is this?",
                                    (QuizMode::Feather, false) => "À qui est cette plume ?",
                                    (_, true) => "Which bird is this?",
                                    (_, false) => "Quel est cet oiseau ?",
                                }}</span>
                                <div class="photo">
                                    <img src=format!("media/{}", m.file) alt="Image to identify"/>
                                    <span class="credit">{format!("{} · {}", m.credit.author, m.credit.license)}</span>
                                </div>
                            })}
                            <div class="options">
                                {options.into_iter().map(|id| {
                                    let cat = cat.clone();
                                    let class = match q.picked {
                                        Some(_) if id == question.target => "opt good",
                                        Some(p) if p == id => "opt bad",
                                        _ => "opt",
                                    };
                                    let latin = cat.species.iter().find(|s| s.id == id).map(|s| s.scientific.clone()).unwrap_or_default();
                                    view! {
                                        <button class=class disabled=answered on:click=move |_| quiz.update(|q| {
                                            if let Some(q) = q {
                                                if q.picked.is_none() {
                                                    q.picked = Some(id);
                                                    let ok = id == question.target;
                                                    progress.update(|p| p.record(mode, question.target, ok));
                                                    if ok { q.score += 100; }
                                                }
                                            }
                                        })>
                                            {move || species_name(&cat, id, lang.get())}
                                            <small>{if answered { latin } else { String::new() }}</small>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                            {answered.then(|| view! {
                                <button class="primary" on:click=move |_| {
                                    let finished = quiz.with(|q| q.as_ref().map(|q| q.idx + 1 >= q.questions.len()).unwrap_or(true));
                                    if finished { screen.set(Screen::Result) } else {
                                        quiz.update(|q| if let Some(q) = q { q.idx += 1; q.picked = None; });
                                    }
                                }>{move || if lang.get() == Lang::En { "Next" } else { "Suivant" }}</button>
                            })}
                        </main>
                    }.into_any()
                }}
            }
        }
    };

    let result_view = {
        let cat = catalog.clone();
        move || {
            let cat = cat.clone();
            let (score, total, mode) = quiz.with(|q| q.as_ref().map(|q| (q.score, q.questions.len(), q.mode)).unwrap_or((0, 0, QuizMode::Photo)));
            view! {
                <main class="screen">
                    <div class="center">
                        <span class="mono">{move || if lang.get() == Lang::En { "Quiz complete" } else { "Quiz terminé" }}</span>
                        <span class="score">{format!("{}/{}", score / 100, total)}</span>
                        <button class="primary" on:click=move |_| { quiz.set(Some(new_quiz(&cat, &progress.get_untracked(), mode, lang.get_untracked(), difficulty.get_untracked(), &regions.get_untracked()))); screen.set(Screen::Quiz); }>
                            {move || if lang.get() == Lang::En { "Play again" } else { "Rejouer" }}
                        </button>
                        <button class="pill" on:click=move |_| screen.set(Screen::Home)>
                            {move || if lang.get() == Lang::En { "Home" } else { "Accueil" }}
                        </button>
                    </div>
                </main>
            }
        }
    };

    move || match screen.get() {
        Screen::Regions => regions_view().into_any(),
        Screen::Home => home().into_any(),
        Screen::Quiz => quiz_view().into_any(),
        Screen::Result => result_view().into_any(),
    }
}

/// Fetches a sound clip into a `blob:` URL, so the player knows its full length.
async fn blob_url(path: &str) -> Option<String> {
    let resp = gloo_net::http::Request::get(path).send().await.ok().filter(|r| r.ok())?;
    let bytes = resp.binary().await.ok()?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes.as_slice()));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("audio/ogg");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
}

async fn load_catalog() -> Result<Catalog, String> {
    let resp = gloo_net::http::Request::get("catalog.json").send().await.map_err(|e| e.to_string())?;
    resp.json::<Catalog>().await.map_err(|e| e.to_string())
}

#[component]
fn Root() -> impl IntoView {
    let catalog = LocalResource::new(load_catalog);
    view! {
        <Suspense fallback=|| view! { <main class="screen"><span class="mono">"Loading…"</span></main> }>
            {move || catalog.get().map(|r| match r {
                Ok(c) => view! { <App catalog=Arc::new(c)/> }.into_any(),
                Err(e) => view! { <main class="screen"><span class="mono">{format!("Could not load catalog: {e}")}</span></main> }.into_any(),
            })}
        </Suspense>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(Root);
}
