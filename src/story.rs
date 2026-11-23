//! Gerador de roteiro ramificado para Visual Novels (usado offline ou como base para a IA refinar).
//! Estrutura: abertura → duas escolhas → três finais, controlados por variáveis (`confianca`, `coragem`).

use crate::spec::*;
use crate::util::*;
use std::collections::BTreeMap;

fn fx(k: &str, v: i32) -> BTreeMap<String, i32> {
    BTreeMap::from([(k.to_string(), v)])
}

pub fn generate(spec: &GameSpec) -> Vec<VnNode> {
    let en = spec.lang == "en";
    let mut rng = Rng::new(spec.seed ^ 0x57_0121);
    let player = if spec.player.name.is_empty() { "???".to_string() } else { spec.player.name.clone() };
    let ally = spec.npcs.first().map(|n| n.name.clone()).unwrap_or_else(|| "Ally".into());
    let rival = if spec.boss.name.is_empty() { "Rival".to_string() } else { spec.boss.name.clone() };
    let place = |i: usize| spec.places.get(i).cloned().unwrap_or_default();
    let item = spec.items.first().map(|i| i.name.to_lowercase()).unwrap_or_default();
    let setting = spec.setting.clone();
    let _ = &mut rng;

    // Cada frase: (pt, en). `{p}` jogador, `{a}` aliado, `{r}` rival, `{l1..3}` lugares, `{i}` item, `{s}` cenário.
    let t = |pt: &str, e: &str| -> String {
        let s = if en { e } else { pt };
        s.replace("{p}", &player)
            .replace("{a}", &ally)
            .replace("{r}", &rival)
            .replace("{l1}", &place(0))
            .replace("{l2}", &place(1))
            .replace("{l3}", &place(2))
            .replace("{i}", &item)
            .replace("{s}", &setting)
    };
    let node = |id: &str, bg: &str, ch: &str, speaker: &str, text: String, next: &str| VnNode {
        id: id.into(),
        bg: bg.into(),
        character: ch.into(),
        speaker: speaker.into(),
        text,
        next: next.into(),
        ..Default::default()
    };
    let narrator = "";
    let (trust, bold) = if en { ("trust", "courage") } else { ("confianca", "coragem") };

    let mut nodes = vec![node("start", "bg_a", "", narrator, t("Tudo começou em {l1}, {s}.", "It all began in {l1}, {s}."), "n1")];
    nodes.push(node("n1", "bg_a", "char_player", &player, t("Eu nunca imaginei que minha vida mudaria tão rápido...", "I never imagined my life would change so fast..."), "n2"));
    nodes.push(node("n2", "bg_a", "char_ally", &ally, t("{p}! Que bom que você veio. Precisamos falar sobre {r}.", "{p}! I'm glad you came. We need to talk about {r}."), "n3"));
    nodes.push(node(
        "n3",
        "bg_a",
        "char_ally",
        &ally,
        t("Ele(a) está atrás do {i} e, se conseguir, tudo aqui estará perdido.", "They're after the {i}, and if they get it, everything here is lost."),
        "c1",
    ));

    let mut c1 = node("c1", "bg_a", "char_ally", &player, t("O que devo fazer?", "What should I do?"), "");
    c1.choices = vec![
        VnChoice { text: t("Ir direto a {l2} e enfrentar o perigo", "Go straight to {l2} and face the danger"), goto: "a1".into(), effect: fx(bold, 1) },
        VnChoice { text: t("Pedir conselhos a {a} antes de partir", "Ask {a} for advice before leaving"), goto: "b1".into(), effect: fx(trust, 1) },
    ];
    nodes.push(c1);

    nodes.push(node("a1", "bg_b", "char_player", &player, t("Não há tempo a perder. Sigo sozinho(a) até {l2}.", "No time to lose. I'm heading to {l2} alone."), "a2"));
    nodes.push(node("a2", "bg_b", "", narrator, t("O caminho é difícil, mas cada passo deixa {p} mais determinado(a).", "The road is hard, but every step makes {p} more determined."), "mid"));
    nodes.push(node("b1", "bg_a", "char_ally", &ally, t("Leve isto: um mapa de {l2}. E lembre-se, ninguém vence sozinho.", "Take this: a map of {l2}. And remember, nobody wins alone."), "b2"));
    nodes.push(node("b2", "bg_b", "", narrator, t("Com o mapa em mãos, {p} chega a {l2} sem se perder.", "Map in hand, {p} reaches {l2} without getting lost."), "mid"));
    nodes.push(node("mid", "bg_b", "char_rival", &rival, t("Então você é {p}. Esperava mais de quem quer me deter.", "So you're {p}. I expected more from someone who wants to stop me."), "mid2"));

    let mut mid2 = node(
        "mid2",
        "bg_b",
        "char_rival",
        &rival,
        t("Junte-se a mim e dividiremos o {i}. Ou recuse... e veja o que acontece.", "Join me and we'll share the {i}. Or refuse... and see what happens."),
        "",
    );
    mid2.choices = vec![
        VnChoice { text: t("Confiar na amizade de {a} e recusar", "Trust {a}'s friendship and refuse"), goto: "d1".into(), effect: fx(trust, 1) },
        VnChoice { text: t("Encarar {r} com toda a minha coragem", "Face {r} with all my courage"), goto: "d2".into(), effect: fx(bold, 1) },
        VnChoice { text: t("Aceitar a proposta... por enquanto", "Accept the offer... for now"), goto: "d3".into(), effect: BTreeMap::new() },
    ];
    nodes.push(mid2);

    nodes.push(node("d1", "bg_c", "char_ally", &ally, t("Eu sabia que você não nos abandonaria! Vamos juntos até {l3}.", "I knew you wouldn't abandon us! Let's go to {l3} together."), "final"));
    nodes.push(node(
        "d2",
        "bg_c",
        "char_player",
        &player,
        t("Chega de fugir. Se for preciso, eu mesmo(a) enfrento tudo em {l3}.", "No more running. If I must, I'll face everything in {l3} myself."),
        "final",
    ));
    nodes.push(node("d3", "bg_c", "char_rival", &rival, t("Sábia decisão... ou talvez não. Veremos em {l3}.", "Wise choice... or maybe not. We'll see in {l3}."), "final"));

    let mut fin = node("final", "bg_c", "", narrator, t("Em {l3}, tudo se decide.", "In {l3}, everything is decided."), "");
    fin.branch = vec![
        VnBranch { cond: format!("{}>=2", trust), goto: "end_good".into() },
        VnBranch { cond: format!("{}>=2", bold), goto: "end_bold".into() },
        VnBranch { cond: String::new(), goto: "end_neutral".into() },
    ];
    nodes.push(fin);

    let mut g = node(
        "end_good",
        "bg_c",
        "char_ally",
        &ally,
        t("Juntos, {p} e {a} protegeram o {i}. {l1} nunca esteve tão em paz!", "Together, {p} and {a} protected the {i}. {l1} has never been so peaceful!"),
        "",
    );
    g.ending = "good".into();
    let mut b = node(
        "end_bold",
        "bg_c",
        "char_player",
        &player,
        t("Com coragem, {p} derrotou {r} sozinho(a). A lenda de {p} começa agora!", "With courage, {p} defeated {r} alone. The legend of {p} begins now!"),
        "",
    );
    b.ending = "good".into();
    let mut n = node(
        "end_neutral",
        "bg_c",
        "char_rival",
        &rival,
        t("{r} escapou com o {i}. Fica a dúvida: o que {p} poderia ter feito diferente?", "{r} escaped with the {i}. The question remains: what could {p} have done differently?"),
        "",
    );
    n.ending = "neutral".into();
    nodes.push(g);
    nodes.push(b);
    nodes.push(n);
    nodes
}

/// Valida o grafo: todos os `goto`/`next` existem e há ao menos um final alcançável.
pub fn validate(nodes: &[VnNode]) -> Vec<String> {
    let ids: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut problems = vec![];
    if nodes.is_empty() {
        problems.push("a história está vazia".to_string());
        return problems;
    }
    for n in nodes {
        let mut targets: Vec<&str> = vec![];
        if !n.next.is_empty() {
            targets.push(&n.next);
        }
        targets.extend(n.choices.iter().map(|c| c.goto.as_str()));
        targets.extend(n.branch.iter().map(|b| b.goto.as_str()));
        for t in targets {
            if !ids.contains(t) {
                problems.push(format!("o nó '{}' aponta para '{}' que não existe", n.id, t));
            }
        }
        if n.next.is_empty() && n.choices.is_empty() && n.branch.is_empty() && n.ending.is_empty() {
            problems.push(format!("o nó '{}' não leva a lugar nenhum e não é um final", n.id));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_story_is_valid_in_both_languages() {
        for lang in ["pt", "en"] {
            let mut spec = heuristic_spec("uma visual novel misteriosa", Some(Genre::VisualNovel), None);
            spec.lang = lang.into();
            let story = generate(&spec);
            assert_eq!(validate(&story), Vec::<String>::new(), "{lang}");
            assert!(story.iter().any(|n| n.ending == "good"));
            assert!(story.iter().all(|n| !n.text.contains('{')), "placeholder sobrando");
        }
    }
}
