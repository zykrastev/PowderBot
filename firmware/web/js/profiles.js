import Api from "./Api.js";

const profileList = document.getElementById("profileList");
const newButton = document.getElementById("newProfileButton");
const saveButton = document.getElementById("saveProfileButton");
const deleteButton = document.getElementById("deleteProfileButton");
const nameInput = document.getElementById("profileName");
const fineStartInput = document.getElementById("fineStartPercent");
const trickleStartInput = document.getElementById("trickleStartPercent");
const stopInput = document.getElementById("stopPercent");
const coarseSpeedInput = document.getElementById("coarseSpeedPercent");
const fineSpeedInput = document.getElementById("fineSpeedPercent");
const trickleSpeedInput = document.getElementById("trickleSpeedPercent");
const settleTimeInput = document.getElementById("settleTimeMs");

let profiles = [];
let selectedProfile = null;

newButton.addEventListener("click", createProfile);
saveButton.addEventListener("click", saveProfile);
deleteButton.addEventListener("click", deleteProfile);

loadProfiles();

async function loadProfiles(selectStorageName = null) {

    try {
        profiles = await Api.listPowders();
        const selected = profiles.find(profile => profile.storageName === selectStorageName);
        selectProfile(selected || profiles[0] || newProfile());
    }
    catch (error) {
        console.error(error);
        profileList.textContent = "Unable to load profiles.";
    }
}

function newProfile() {

    const id = `p${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
    return {
        version: 1,
        id,
        storageName: `profile_${id}`,
        name: "New powder",
        fineStartPercent: 75.0,
        trickleStartPercent: 97.0,
        stopPercent: 99.5,
        coarseSpeedPercent: 100.0,
        fineSpeedPercent: 30.0,
        trickleSpeedPercent: 5.0,
        settleTimeMs: 300,
        isNew: true
    };
}

function createProfile() {

    selectProfile(newProfile());
    nameInput.focus();
    nameInput.select();
}

function selectProfile(profile) {

    selectedProfile = profile;
    nameInput.value = profile.name;
    fineStartInput.value = profile.fineStartPercent;
    trickleStartInput.value = profile.trickleStartPercent;
    stopInput.value = profile.stopPercent;
    coarseSpeedInput.value = profile.coarseSpeedPercent;
    fineSpeedInput.value = profile.fineSpeedPercent;
    trickleSpeedInput.value = profile.trickleSpeedPercent;
    settleTimeInput.value = profile.settleTimeMs;
    deleteButton.disabled = profile.isNew;
    renderProfileList();
}

function renderProfileList() {

    profileList.replaceChildren();

    profiles.forEach(profile => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "profile-item";
        button.classList.toggle("active", profile.storageName === selectedProfile.storageName);

        const title = document.createElement("strong");
        title.textContent = profile.name;
        const details = document.createElement("span");
        details.textContent = `F ${profile.fineStartPercent}% / T ${profile.trickleStartPercent}% / S ${profile.stopPercent}%`;
        button.append(title, details);
        button.addEventListener("click", () => selectProfile(profile));
        profileList.append(button);
    });
}

function readForm() {

    const profile = {
        ...selectedProfile,
        name: nameInput.value.trim(),
        fineStartPercent: Number(fineStartInput.value),
        trickleStartPercent: Number(trickleStartInput.value),
        stopPercent: Number(stopInput.value),
        coarseSpeedPercent: Number(coarseSpeedInput.value),
        fineSpeedPercent: Number(fineSpeedInput.value),
        trickleSpeedPercent: Number(trickleSpeedInput.value),
        settleTimeMs: Number(settleTimeInput.value)
    };

    if (!profile.name || new TextEncoder().encode(profile.name).length > 64 ||
        !isValidPercent(profile.fineStartPercent) ||
        !isValidPercent(profile.trickleStartPercent) ||
        !isValidPercent(profile.stopPercent) ||
        profile.fineStartPercent > profile.trickleStartPercent ||
        profile.trickleStartPercent > profile.stopPercent ||
        !isValidPercent(profile.coarseSpeedPercent) ||
        !isValidPercent(profile.fineSpeedPercent) ||
        !isValidPercent(profile.trickleSpeedPercent) ||
        !Number.isInteger(profile.settleTimeMs) || profile.settleTimeMs < 0 || profile.settleTimeMs > 4294967295) {
        throw new Error("Enter percentages from 0 to 100, ordered start/stop thresholds, and a whole-number settle time in milliseconds.");
    }

    delete profile.isNew;
    return profile;
}

function isValidPercent(value) {

    return Number.isFinite(value) && value >= 0 && value <= 100;
}

async function saveProfile() {

    try {
        const profile = readForm();
        saveButton.disabled = true;

        if (selectedProfile.isNew)
            await Api.createPowder(profile);
        else
            await Api.updatePowder(profile);

        await loadProfiles(profile.storageName);
    }
    catch (error) {
        window.alert(error.message);
    }
    finally {
        saveButton.disabled = false;
    }
}

async function deleteProfile() {

    if (selectedProfile.isNew || !window.confirm(`Delete '${selectedProfile.name}'?`))
        return;

    try {
        deleteButton.disabled = true;
        await Api.deletePowder(selectedProfile.storageName);
        await loadProfiles();
    }
    catch (error) {
        window.alert(error.message);
    }
    finally {
        deleteButton.disabled = false;
    }
}
