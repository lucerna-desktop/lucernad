//! O compositor no modo jogo (D4 a D7, D13 e D18): desfoque, sombras e
//! animações desligados, o VRR em tela cheia e o tearing opcional, com o
//! diário gravado antes de qualquer mudança e a restauração exata, opção por
//! opção.

use std::collections::BTreeSet;
use std::path::Path;

use lucernad_hyprland::options::OptionValue;
use lucernad_hyprland::socket::HyprError;
use lucernad_hyprland::{Hypr, SocketHypr, get_option, instance, set_options, set_window_prop};

use crate::config::GameSettings;
use crate::journal::{Entry, Journal, Store};

pub const BLUR: &str = "decoration:blur:enabled";
pub const SHADOW: &str = "decoration:shadow:enabled";
pub const ANIMATIONS: &str = "animations:enabled";
pub const VRR: &str = "misc:vrr";
pub const TEARING: &str = "general:allow_tearing";

/// As opções que o modo jogo toca, exatamente estas (D4).
pub const OPTIONS: [&str; 5] = [BLUR, SHADOW, ANIMATIONS, VRR, TEARING];

/// A propriedade de janela do tearing.
const IMMEDIATE: &str = "immediate";

/// O que o modo jogo lembra entre as reaplicações de um mesmo modo ativo
/// (vazio a cada entrada): os valores que viu depois da última mudança e as
/// opções devolvidas ao usuário, que ele mudou no meio do jogo e que um
/// `configure` ou uma troca das janelas do tearing não replaneja até o modo
/// sair. Um reload devolve tudo ao arquivo e o modo jogo reaplica tudo (D5);
/// um Hyprland reiniciado (outra instância) começa a sessão do zero. A opção
/// devolvida sai do diário de vez: se o usuário a puser de novo no valor que
/// o modo jogo tinha posto, ela fica assim ao sair (sem reaplicação no meio,
/// o diário antigo a devolveria ao antes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    /// A instância do Hyprland em que o `seen` foi visto.
    instance: Option<String>,
    seen: Vec<(String, OptionValue)>,
    released: BTreeSet<String>,
}

impl Session {
    /// As opções devolvidas ao usuário.
    pub fn released(&self) -> &BTreeSet<String> {
        &self.released
    }
}

/// O que mudar, a partir dos valores `current` (os de antes): os efeitos em
/// falso; o VRR em 2 só se estiver em 0 (D6); o tearing só se pedido (D7).
/// O que já está no valor pedido fica de fora.
pub fn plan(current: &[(String, OptionValue)], s: &GameSettings) -> Vec<Entry> {
    let mut out = Vec::new();
    for (option, before) in current {
        let target = match option.as_str() {
            BLUR | SHADOW | ANIMATIONS if s.effects => Some(OptionValue::Bool(false)),
            VRR if s.vrr && *before == OptionValue::Int(0) => Some(OptionValue::Int(2)),
            TEARING if s.tearing => Some(OptionValue::Bool(true)),
            _ => None,
        };
        if let Some(applied) = target.filter(|t| t != before) {
            out.push(Entry {
                option: option.clone(),
                before: before.clone(),
                applied,
            });
        }
    }
    out
}

/// Liga o que `s` pede (ou acerta a diferença com o que já foi posto):
/// lê as opções, monta o "antes" (o do diário para a opção que ainda está
/// com o valor posto; o atual para as outras), **grava o diário**, muda
/// tudo num `eval` só, desfaz o que deixou de ser pedido e acerta o
/// `immediate` das `tearing_windows`. A opção que o usuário mudou desde a
/// última vez (pela `session`) fica com ele: sai do diário e do plano.
/// Devolve o diário (`None` sem nada a desfazer, e então sem arquivo).
pub async fn apply(
    hypr: &dyn Hypr,
    store: &Store,
    s: &GameSettings,
    tearing_windows: &[String],
    session: &mut Session,
) -> Result<Option<Journal>, HyprError> {
    reconcile(hypr, store, s, tearing_windows, session, false).await
}

/// O [`apply`] depois de um `configreloaded`: o reload pôs tudo de volta ao
/// arquivo, então a opção com outro valor ganha o novo "antes"; a que voltou
/// no mesmo valor que o modo jogo pôs fica com o "antes" do diário (D5,
/// decisão do usuário: do lado seguro, mesmo com a corrida entre o reload e o
/// evento). Ao contrário do [`apply`], nada fica com o usuário: as opções
/// devolvidas voltam ao plano.
pub async fn apply_after_reload(
    hypr: &dyn Hypr,
    store: &Store,
    s: &GameSettings,
    tearing_windows: &[String],
    session: &mut Session,
) -> Result<Option<Journal>, HyprError> {
    reconcile(hypr, store, s, tearing_windows, session, true).await
}

fn save(store: &Store, j: &Journal) -> Result<(), HyprError> {
    store
        .save(j)
        .map_err(|e| HyprError::Refused(format!("não consegui gravar o diário do modo jogo: {e}")))
}

async fn reconcile(
    hypr: &dyn Hypr,
    store: &Store,
    s: &GameSettings,
    tearing_windows: &[String],
    session: &mut Session,
    reload: bool,
) -> Result<Option<Journal>, HyprError> {
    let mut now = Vec::new();
    for o in OPTIONS {
        now.push((o.to_string(), get_option(hypr, o).await?));
    }
    let instance = hypr
        .instance()
        .ok_or_else(|| HyprError::Unreachable("sem instância do Hyprland".into()))?;
    let prev = store.load().filter(|j| j.instance == instance);
    if session.instance.as_deref() != Some(instance.as_str()) {
        // Outra instância: os valores dela são os do arquivo, não do usuário.
        *session = Session {
            instance: Some(instance.clone()),
            ..Session::default()
        };
    }
    if reload {
        session.released.clear();
    } else {
        // O que mudou desde o que o modo jogo viu foi o usuário: fica com ele.
        for (o, cur) in &now {
            if session.seen.iter().any(|(so, sv)| so == o && sv != cur) {
                session.released.insert(o.clone());
            }
        }
    }
    // Até o eval dar certo, nada de comparar com o que se viu.
    session.seen.clear();
    let released = &session.released;
    // O "antes" do diário vale para a opção que está com o valor posto,
    // inclusive depois de um reload (D5, decisão do usuário): um reload que
    // devolve o mesmo valor, ou um reconcile que já reaplicou antes do
    // `configreloaded`, nunca apaga o valor de antes.
    let prev_entries: Vec<Entry> = prev
        .as_ref()
        .map_or(&[][..], |j| &j.entries[..])
        .iter()
        .filter(|e| !released.contains(&e.option))
        .cloned()
        .collect();
    let ours = |o: &str, cur: &OptionValue| {
        prev_entries
            .iter()
            .find(|e| e.option == o && &e.applied == cur)
    };
    let baseline: Vec<(String, OptionValue)> = now
        .iter()
        .filter(|(o, _)| !released.contains(o))
        .map(|(o, cur)| {
            let before = ours(o, cur).map_or(cur, |e| &e.before);
            (o.clone(), before.clone())
        })
        .collect();
    let entries = plan(&baseline, s);
    // O que foi posto e não é mais pedido: volta ao antes.
    let reverts: Vec<Entry> = prev_entries
        .iter()
        .filter(|e| !entries.iter().any(|n| n.option == e.option))
        .filter(|e| {
            now.iter()
                .any(|(o, cur)| *o == e.option && *cur == e.applied)
        })
        .cloned()
        .collect();
    let wanted_tw: Vec<String> = if s.tearing {
        tearing_windows.to_vec()
    } else {
        Vec::new()
    };
    let prev_tw: Vec<String> = prev
        .as_ref()
        .map(|j| j.tearing_windows.clone())
        .unwrap_or_default();
    let to_on: Vec<&String> = wanted_tw.iter().filter(|w| !prev_tw.contains(w)).collect();
    let to_off: Vec<&String> = prev_tw.iter().filter(|w| !wanted_tw.contains(w)).collect();

    let target = Journal {
        instance: instance.clone(),
        entries: entries.clone(),
        tearing_windows: wanted_tw.clone(),
    };
    // Antes do eval: o diário cobre o que vai ser posto e o que ainda não
    // voltou.
    let mut during = target.clone();
    during.entries.extend(reverts.iter().cloned());
    for w in &to_off {
        during.tearing_windows.push((*w).clone());
    }
    if !during.is_empty() {
        save(store, &during)?;
    }
    let mut changes: Vec<(&str, OptionValue)> = Vec::new();
    for e in &entries {
        let cur = now.iter().find(|(o, _)| *o == e.option).map(|(_, v)| v);
        if cur != Some(&e.applied) {
            changes.push((e.option.as_str(), e.applied.clone()));
        }
    }
    for e in &reverts {
        changes.push((e.option.as_str(), e.before.clone()));
    }
    // A ordem fixa das opções (um eval previsível).
    changes.sort_by_key(|(o, _)| OPTIONS.iter().position(|x| x == o));
    set_options(hypr, &changes).await?;
    for w in to_on {
        set_window_prop(hypr, w, IMMEDIATE, "1").await?;
    }
    for w in to_off {
        set_window_prop(hypr, w, IMMEDIATE, "0").await?;
    }
    session.seen = now
        .into_iter()
        .map(|(o, v)| {
            let v = changes
                .iter()
                .find(|(c, _)| *c == o)
                .map_or(v, |(_, n)| n.clone());
            (o, v)
        })
        .collect();
    if target.is_empty() {
        store.clear();
        Ok(None)
    } else {
        save(store, &target)?;
        Ok(Some(target))
    }
}

/// Desfaz o diário `journal` (D5): relê cada opção e só devolve a que ainda
/// está com o valor posto (a mudança do usuário vence); desliga o
/// `immediate` das janelas; apaga o diário só se tudo deu certo. Diário de
/// outra instância (o Hyprland reiniciou e já leu a config do zero) é só
/// apagado.
pub async fn restore(hypr: &dyn Hypr, store: &Store, journal: &Journal) -> Result<(), HyprError> {
    let first = journal
        .entries
        .first()
        .map(|e| e.option.as_str())
        .unwrap_or(ANIMATIONS);
    let mut now = vec![(first.to_string(), get_option(hypr, first).await?)];
    if hypr.instance().as_deref() != Some(journal.instance.as_str()) {
        eprintln!(
            "lucernad: o diário do modo jogo era de outra instância do Hyprland ({}): apagado sem restaurar",
            journal.instance
        );
        store.clear();
        return Ok(());
    }
    for e in journal.entries.iter().skip(1) {
        now.push((e.option.clone(), get_option(hypr, &e.option).await?));
    }
    // A opção que não está mais com o valor posto ficou com o usuário (M3):
    // sai do diário antes do eval, para uma nova tentativa não a devolver ao
    // antes se ele a puser depois no mesmo valor que o modo jogo tinha posto.
    let ours = |e: &Entry| now.iter().any(|(o, v)| *o == e.option && *v == e.applied);
    if !journal.entries.iter().all(ours) {
        let mut left = journal.clone();
        left.entries.retain(ours);
        if left.is_empty() {
            // Nada mais a desfazer: sem diário vazio segurando o daemon.
            store.clear();
            return Ok(());
        }
        save(store, &left)?;
    }
    let mut changes: Vec<(&str, OptionValue)> = journal
        .entries
        .iter()
        .filter(|e| ours(e))
        .map(|e| (e.option.as_str(), e.before.clone()))
        .collect();
    changes.sort_by_key(|(o, _)| OPTIONS.iter().position(|x| x == o));
    set_options(hypr, &changes).await?;
    for w in &journal.tearing_windows {
        set_window_prop(hypr, w, IMMEDIATE, "0").await?;
    }
    store.clear();
    Ok(())
}

/// O `lucernad restaurar-jogo` (o `ExecStopPost` do serviço): sem daemon,
/// lê o diário do ambiente e fala direto com o socket do Hyprland. Sai 0
/// sem diário; 1 com erro (o diário fica).
pub async fn restore_from_journal_cli() -> i32 {
    let store = match Store::from_env() {
        Ok(s) => s,
        // Sem XDG_RUNTIME_DIR não há diário.
        Err(_) => return 0,
    };
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) else {
        return 0;
    };
    restore_cli_at(&store, Path::new(&runtime)).await
}

/// O [`restore_from_journal_cli`] com o diário em `store` e o Hyprland em
/// `runtime_dir/hypr`.
pub async fn restore_cli_at(store: &Store, runtime_dir: &Path) -> i32 {
    let Some(j) = store.load() else {
        return 0;
    };
    if instance::find(runtime_dir, Some(&j.instance)).is_err() {
        eprintln!(
            "lucernad: o diário do modo jogo era de um Hyprland que já saiu ({}): apagado",
            j.instance
        );
        store.clear();
        return 0;
    }
    let hypr = SocketHypr::at(runtime_dir, Some(&j.instance));
    match restore(&hypr, store, &j).await {
        Ok(()) => {
            eprintln!("lucernad: o modo jogo foi desfeito pelo diário");
            0
        }
        Err(e) => {
            eprintln!("erro: não consegui desfazer o modo jogo: {e}");
            1
        }
    }
}
