/* Adapted from millida/launcher src/vendor/mine3d/core/skin-animations.ts; GPL-3.0-only. See MILLIDA-LICENSE and MILLIDA-NOTICE.md. */
window.MillidaAnimations=(()=>{const exports={};
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.create = exports.DanceAnimation = exports.SadAnimation = exports.VictoryAnimation = exports.CoolPoseAnimation = exports.LookAroundAnimation = exports.SneakAnimation = exports.WaveHelloAnimation = exports.TrailerRunAnimation = exports.HeroIdleAnimation = void 0;
exports.animationControlsLegs = animationControlsLegs;
const punch = (x) => Math.sin(x) * Math.abs(Math.sin(x));
class BaseSkinAnimation {
    constructor() {
        this.speed = 1;
        this.paused = false;
        this.progress = 0;
        this.controlsLegs = false;
    }
    update(player, deltaTime) {
        if (this.paused)
            return;
        const delta = deltaTime * this.speed;
        this.animate(player, delta);
        this.progress += delta;
    }
}
function animationControlsLegs(animation) {
    return Boolean(animation?.controlsLegs);
}
/**
 * Trailer idle — «живой» герой: дыхание, slim/classic осанка, толчок по клику.
 */
const NUDGE_RAMP_IN = 0.2;
const NUDGE_HOLD_UNTIL = 0.34;
const NUDGE_LOOK_RELEASE = 0.68;
class HeroIdleAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
        /**
         * Движок включает при взгляде за курсором: анимация не крутит голову,
         * иначе взгляд и idle-look конфликтуют и выглядят криво.
         */
        this.suppressAutoLook = false;
        /** Тонкая модель — чуть другая осанка */
        this.modelSlim = false;
        this._nudgeElapsed = -1;
        this._nudgeImpactPending = false;
    }
    nudge() {
        if (this.isNudging)
            return false;
        this._nudgeElapsed = 0;
        this._nudgeImpactPending = true;
        return true;
    }
    get isNudging() {
        return this._nudgeElapsed >= 0;
    }
    /** Блокирует взгляд курсором во время толчка */
    get blocksCursorLook() {
        return this.isNudging && this._nudgeElapsed < HeroIdleAnimation.NUDGE_DURATION * NUDGE_LOOK_RELEASE;
    }
    consumeNudgeImpact() {
        const v = this._nudgeImpactPending;
        this._nudgeImpactPending = false;
        return v;
    }
    animate(player, delta) {
        const t = this.progress;
        const breathe = Math.sin(t * 1.35);
        const wind = Math.sin(t * 2.1) * 0.5 + Math.sin(t * 3.4) * 0.5;
        const weight = 0.55 + Math.sin(t * 0.35) * 0.45;
        const slim = this.modelSlim ? 1 : 0;
        // Спокойная стойка: лёгкое дыхание, руки у тела (не «распахнуты»)
        player.skin.body.rotation.x = 0.02 + breathe * 0.025 - slim * 0.01;
        player.skin.body.rotation.z = (weight - 0.5) * (0.04 + slim * 0.02);
        const busyHead = this.blocksCursorLook;
        if (this.suppressAutoLook && !busyHead) {
            player.skin.body.rotation.y = -0.04 + slim * 0.02;
            player.skin.head.rotation.set(-0.03, 0, 0);
        }
        else {
            const lookRaw = Math.sin(t * 0.55);
            const look = lookRaw * lookRaw * lookRaw;
            player.skin.body.rotation.y = -0.06 + look * 0.06 + slim * 0.02;
            player.skin.head.rotation.y = look * 0.4;
            player.skin.head.rotation.x = -0.04 + Math.sin(t * 0.8) * 0.05;
            player.skin.head.rotation.z = look * 0.03 + slim * 0.02;
        }
        // Руки почти вдоль тела; z ≈ ±0.06 — естественный зазор, не разведение
        player.skin.leftArm.rotation.x = -0.05 + breathe * 0.035 + slim * 0.02;
        player.skin.leftArm.rotation.z = 0.06 + wind * 0.015 - slim * 0.015;
        player.skin.rightArm.rotation.x = -0.03 + Math.sin(t * 0.9 + 1.2) * 0.04;
        player.skin.rightArm.rotation.z = -0.06 - weight * 0.02 + slim * 0.015;
        // Ноги почти параллельно — ощущение «стоит», а не контрапоста
        player.skin.leftLeg.rotation.x = -0.03 * weight;
        player.skin.rightLeg.rotation.x = 0.04 * weight + slim * 0.02;
        player.skin.leftLeg.rotation.z = 0.015;
        player.skin.rightLeg.rotation.z = -0.015;
        player.cape.rotation.x = Math.PI * 0.1 + wind * 0.06 + breathe * 0.02;
        if (this._nudgeElapsed >= 0) {
            this._nudgeElapsed += delta;
            const u = this._nudgeElapsed / HeroIdleAnimation.NUDGE_DURATION;
            if (u >= 1)
                this._nudgeElapsed = -1;
            else
                this._applyNudgeOverlay(player, u);
        }
    }
    _applyNudgeOverlay(player, u) {
        let push;
        if (u < NUDGE_RAMP_IN) {
            const x = u / NUDGE_RAMP_IN;
            push = x * x * (3 - 2 * x);
        }
        else if (u < NUDGE_HOLD_UNTIL) {
            push = 1;
        }
        else {
            const x = (u - NUDGE_HOLD_UNTIL) / (1 - NUDGE_HOLD_UNTIL);
            push = 1 - x * x * (3 - 2 * x);
        }
        const shakeEnv = push * (u < 0.55 ? 1 : Math.max(0, 1 - (u - 0.55) / 0.3));
        const shake = Math.sin(u * Math.PI * 11) * shakeEnv;
        player.position.z -= 1.55 * push;
        player.position.y += 0.2 * push;
        player.rotation.x -= 0.32 * push;
        player.rotation.z += shake * 0.05;
        player.skin.body.rotation.x -= 0.18 * push;
        player.skin.body.rotation.z += shake * 0.04;
        player.skin.head.rotation.y += shake * 0.42;
        player.skin.head.rotation.z += shake * 0.22;
        player.skin.head.rotation.x -= 0.1 * push + Math.abs(shake) * 0.06;
        player.skin.leftArm.rotation.x -= 0.55 * push;
        player.skin.rightArm.rotation.x -= 0.5 * push;
        player.skin.leftArm.rotation.z += 0.35 * push;
        player.skin.rightArm.rotation.z -= 0.35 * push;
        player.skin.leftLeg.rotation.x -= 0.18 * push;
        player.skin.rightLeg.rotation.x += 0.22 * push;
        player.cape.rotation.x += 0.12 * push;
    }
}
exports.HeroIdleAnimation = HeroIdleAnimation;
HeroIdleAnimation.NUDGE_DURATION = 1.6;
/**
 * Trailer sprint — широкий шаг, сильный наклон, плащ парусом,
 * противоположный мах рук; темп чуть выше среднего игрового спринта.
 */
class TrailerRunAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress * 9.6;
        const stride = Math.sin(t);
        const strideOpp = Math.sin(t + Math.PI);
        const plant = punch(t);
        const plantOpp = punch(t + Math.PI);
        const bob = Math.abs(Math.sin(t));
        player.rotation.x = 0.26;
        player.rotation.z = stride * 0.035;
        player.position.y = bob * 0.55;
        player.position.x = stride * 0.08;
        player.skin.body.rotation.x = 0.12;
        player.skin.body.rotation.y = stride * 0.12;
        player.skin.body.rotation.z = stride * 0.04;
        player.skin.head.rotation.x = -0.18;
        player.skin.head.rotation.y = stride * 0.07;
        player.skin.head.rotation.z = -stride * 0.03;
        // Ноги — широкий театральный шаг
        player.skin.leftLeg.rotation.x = strideOpp * 1.15;
        player.skin.rightLeg.rotation.x = stride * 1.15;
        player.skin.leftLeg.rotation.z = plantOpp * 0.06;
        player.skin.rightLeg.rotation.z = -plant * 0.06;
        // Руки — противоположный мах с раскрытием в стороны
        player.skin.leftArm.rotation.x = stride * 1.25;
        player.skin.rightArm.rotation.x = strideOpp * 1.25;
        player.skin.leftArm.rotation.z = 0.22 + Math.abs(stride) * 0.12;
        player.skin.rightArm.rotation.z = -0.22 - Math.abs(strideOpp) * 0.12;
        player.cape.rotation.x = Math.PI * 0.42 + bob * 0.14;
    }
}
exports.TrailerRunAnimation = TrailerRunAnimation;
/** Приветствие — мягкое и милое: лёгкий подскок, наклон головы, ласковый мах */
class WaveHelloAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress * 5.0;
        const wave = Math.sin(t);
        // Мягкий «радостный» подскок в такт маху
        const bounce = 0.5 - 0.5 * Math.cos(t);
        player.position.y = bounce * 0.35;
        player.rotation.z = wave * 0.025;
        // Голова чуть набок и к камере — «привет!»
        player.skin.head.rotation.x = -0.1 + bounce * 0.03;
        player.skin.head.rotation.y = 0.22 + wave * 0.08;
        player.skin.head.rotation.z = 0.16 + wave * 0.06;
        player.skin.body.rotation.x = 0.05;
        player.skin.body.rotation.y = 0.12;
        player.skin.body.rotation.z = -0.05 + wave * 0.03;
        // Рука поднята не до предела и мягко качается — без «сломанного» плеча
        player.skin.rightArm.rotation.x = -0.2 + wave * 0.12;
        player.skin.rightArm.rotation.y = 0.2;
        player.skin.rightArm.rotation.z = -Math.PI * 0.58 + wave * 0.32;
        // Вторая рука чуть прижата, тоже чуть оживает
        player.skin.leftArm.rotation.x = -0.22;
        player.skin.leftArm.rotation.y = 0;
        player.skin.leftArm.rotation.z = 0.18 + bounce * 0.05;
        // Вес на одну ногу — милая стойка
        player.skin.leftLeg.rotation.set(-0.08, 0, 0.04);
        player.skin.rightLeg.rotation.set(0.12, 0, -0.05);
        player.cape.rotation.x = Math.PI * 0.09 + bounce * 0.03;
    }
}
exports.WaveHelloAnimation = WaveHelloAnimation;
/** Красться — низкий шаг, взгляд строго вперёд по направлению движения */
class SneakAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress * 3.8;
        const stride = Math.sin(t);
        const tense = 0.5 + Math.sin(t * 2.2) * 0.5;
        player.position.y = -1.5;
        // Наклон вперёд как у бега — смотрит туда же, куда идёт
        player.rotation.x = 0.2;
        player.skin.body.rotation.x = 0.16;
        player.skin.body.rotation.y = stride * 0.05;
        player.skin.body.rotation.z = 0;
        // Голова вперёд/чуть вниз на путь — без поворотов «назад»
        player.skin.head.rotation.x = -0.1;
        player.skin.head.rotation.y = Math.sin(t * 0.45) * 0.06;
        player.skin.head.rotation.z = 0;
        player.skin.leftLeg.rotation.x = stride * 0.5;
        player.skin.rightLeg.rotation.x = -stride * 0.5;
        player.skin.leftArm.rotation.x = -stride * 0.4 - 0.28;
        player.skin.rightArm.rotation.x = stride * 0.4 - 0.28;
        player.skin.leftArm.rotation.z = 0.18 + tense * 0.05;
        player.skin.rightArm.rotation.z = -0.18 - tense * 0.05;
        player.cape.rotation.x = Math.PI * 0.2;
    }
}
exports.SneakAnimation = SneakAnimation;
/** Обзор — резкие взгляды по сторонам с «паузами» на краях */
class LookAroundAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress * 0.85;
        const lookRaw = Math.sin(t);
        const look = lookRaw * Math.abs(lookRaw) * Math.abs(lookRaw);
        const peek = Math.sin(t * 2.4);
        player.skin.head.rotation.y = look * 0.95;
        player.skin.head.rotation.x = -0.08 + peek * 0.1;
        player.skin.head.rotation.z = look * 0.06;
        player.skin.body.rotation.y = look * 0.28;
        player.skin.body.rotation.z = -look * 0.04;
        player.skin.leftArm.rotation.x = -0.2 - Math.abs(look) * 0.1;
        player.skin.rightArm.rotation.x = -0.15 - Math.abs(look) * 0.08;
        player.skin.leftArm.rotation.z = 0.14 + Math.abs(look) * 0.08;
        player.skin.rightArm.rotation.z = -0.14 - Math.abs(look) * 0.08;
        player.skin.leftLeg.rotation.x = -0.06;
        player.skin.rightLeg.rotation.x = 0.1;
        player.cape.rotation.x = Math.PI * 0.1 + Math.abs(look) * 0.03;
    }
}
exports.LookAroundAnimation = LookAroundAnimation;
/** Крутая стойка — вес на бедре, рука на груди, взгляд в камеру */
class CoolPoseAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress;
        const breathe = Math.sin(t * 1.25);
        player.skin.body.rotation.y = -0.32;
        player.skin.body.rotation.z = 0.08;
        player.skin.body.rotation.x = 0.05 + breathe * 0.02;
        player.skin.head.rotation.y = 0.48;
        player.skin.head.rotation.x = -0.12 + breathe * 0.025;
        player.skin.head.rotation.z = 0.05;
        player.skin.leftArm.rotation.x = -1.05;
        player.skin.leftArm.rotation.z = 0.42;
        player.skin.leftArm.rotation.y = 0.25;
        player.skin.rightArm.rotation.x = -0.55;
        player.skin.rightArm.rotation.z = -0.55;
        player.skin.rightArm.rotation.y = -0.15;
        player.skin.leftLeg.rotation.x = -0.05;
        player.skin.rightLeg.rotation.x = 0.18;
        player.skin.rightLeg.rotation.z = -0.08;
        player.cape.rotation.x = Math.PI * 0.14 + breathe * 0.03;
    }
}
exports.CoolPoseAnimation = CoolPoseAnimation;
/** Победа — руки вверх, подскоки */
class VictoryAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress * 6.2;
        const bounce = 0.5 - 0.5 * Math.cos(t);
        const sway = Math.sin(t);
        player.position.y = bounce * 1.1;
        player.rotation.z = sway * 0.04;
        player.skin.head.rotation.x = -0.15 + bounce * 0.05;
        player.skin.head.rotation.y = sway * 0.1;
        player.skin.body.rotation.x = -0.06;
        player.skin.body.rotation.y = sway * 0.06;
        player.skin.leftArm.rotation.z = 2.35 + sway * 0.12;
        player.skin.rightArm.rotation.z = -2.35 - sway * 0.12;
        player.skin.leftArm.rotation.x = -0.15 + bounce * 0.1;
        player.skin.rightArm.rotation.x = -0.15 + bounce * 0.1;
        player.skin.leftLeg.rotation.x = -0.15 * bounce;
        player.skin.rightLeg.rotation.x = 0.2 * bounce;
        player.cape.rotation.x = Math.PI * 0.12 + bounce * 0.08;
    }
}
exports.VictoryAnimation = VictoryAnimation;
/** Грусть — плечи опущены, взгляд в пол */
class SadAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress;
        const sigh = Math.sin(t * 1.1);
        player.skin.head.rotation.x = 0.42 + sigh * 0.04;
        player.skin.head.rotation.y = -0.12;
        player.skin.head.rotation.z = -0.08;
        player.skin.body.rotation.x = 0.18;
        player.skin.body.rotation.z = -0.04;
        player.skin.body.rotation.y = 0.06;
        player.skin.leftArm.rotation.x = 0.15;
        player.skin.rightArm.rotation.x = 0.2;
        player.skin.leftArm.rotation.z = 0.08;
        player.skin.rightArm.rotation.z = -0.06;
        player.skin.leftArm.rotation.y = 0.05;
        player.skin.rightArm.rotation.y = -0.05;
        player.skin.leftLeg.rotation.x = -0.05;
        player.skin.rightLeg.rotation.x = 0.08;
        player.cape.rotation.x = Math.PI * 0.06 + sigh * 0.015;
    }
}
exports.SadAnimation = SadAnimation;
/** Танец — ритмичные шаги и мах руками */
class DanceAnimation extends BaseSkinAnimation {
    constructor() {
        super(...arguments);
        this.controlsLegs = true;
    }
    animate(player) {
        const t = this.progress * 7.4;
        const beat = Math.sin(t);
        const beatOpp = Math.sin(t + Math.PI);
        const bob = Math.abs(Math.sin(t * 2));
        player.position.y = bob * 0.65;
        player.rotation.z = beat * 0.08;
        player.skin.body.rotation.y = beat * 0.28;
        player.skin.body.rotation.z = beat * 0.06;
        player.skin.body.rotation.x = 0.06;
        player.skin.head.rotation.y = beat * 0.2;
        player.skin.head.rotation.z = -beat * 0.08;
        player.skin.head.rotation.x = -0.08 + bob * 0.05;
        player.skin.leftArm.rotation.x = beatOpp * 0.9;
        player.skin.rightArm.rotation.x = beat * 0.9;
        player.skin.leftArm.rotation.z = 0.55 + bob * 0.25;
        player.skin.rightArm.rotation.z = -0.55 - bob * 0.25;
        player.skin.leftLeg.rotation.x = beat * 0.55;
        player.skin.rightLeg.rotation.x = beatOpp * 0.55;
        player.skin.leftLeg.rotation.z = 0.06;
        player.skin.rightLeg.rotation.z = -0.06;
        player.cape.rotation.x = Math.PI * 0.16 + bob * 0.1;
    }
}
exports.DanceAnimation = DanceAnimation;
const create = (id) => new ({ idle: HeroIdleAnimation, run: TrailerRunAnimation, wave: WaveHelloAnimation, sneak: SneakAnimation, look: LookAroundAnimation, cool: CoolPoseAnimation, victory: VictoryAnimation, sad: SadAnimation, dance: DanceAnimation }[id] || HeroIdleAnimation)();
exports.create = create;

return exports;})();
