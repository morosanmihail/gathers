import React from "react";
import { OperationsProvider, ModeProvider } from "./OperationsContext";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import CardListView from "./Views/CardListView";
import SearchView from "./Views/SearchView";
import SettingsView from "./Views/SettingsView";
import MtgCardDetailView from "./Views/MtgCardDetailView";
import RiftboundCardDetailView from "./Views/RiftboundCardDetailView";
import PokemonCardDetailView from "./Views/PokemonCardDetailView";

export default function BaseApp({ mode = "full", collectionsEnabled = false }) {
  return (
    <ModeProvider mode={mode} collectionsEnabled={collectionsEnabled}>
      <OperationsProvider>
        <BrowserRouter>
          <Routes>
            <Route path="/" element={<Navigate to="/search" />} />
            <Route path="/search" element={<SearchView />} />
            <Route path="/settings" element={<SettingsView />} />
            <Route path="/card/mtg/:id" element={<MtgCardDetailView />} />
            <Route path="/card/riftbound/:id" element={<RiftboundCardDetailView />} />
            <Route path="/card/pokemon/:id" element={<PokemonCardDetailView />} />
            {collectionsEnabled ? (
              <Route path="/c/:collection">
                <Route index element={<Navigate to="1" replace />} />
                <Route path=":pageNumber" element={<CardListView />} />
              </Route>
            ) : (
              <Route path="/c/*" element={<Navigate to="/search" />} />
            )}
          </Routes>
        </BrowserRouter>
      </OperationsProvider>
    </ModeProvider>
  );
}
