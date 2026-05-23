import React from "react";
import { CardCacheProvider } from "../Components/CardListContexts/CardCacheContext";
import { CardLoaderProvider } from "../Components/CardListContexts/CardLoaderContext";
import { CardSetsProvider } from "../Components/ReusableConstants/CardSets";
import { SelectedCardsProvider } from "../Components/CardListContexts/SelectedCardsContext";
import { CardsProvider } from "../Components/CardListContexts/CardsContext";
import { CollectionsProvider } from "../Components/CollectionContext";
import Header from "../Components/Layout/Header";
import { PricesProvider } from "../Components/CardListContexts/PricesContext";
import { RefreshCardListProvider } from "../Components/CardListContexts/RefreshCardListContext";
import { SystemTypeProvider } from "../Components/SystemTypeContext";

export default function ViewProviders({ children }) {
  return (
    <CollectionsProvider>
      <SystemTypeProvider>
        <Header />
        <main>
          <CardsProvider>
            <PricesProvider>
              <SelectedCardsProvider>
                <CardCacheProvider>
                  <CardLoaderProvider>
                    <RefreshCardListProvider>
                      <CardSetsProvider>{children}</CardSetsProvider>
                    </RefreshCardListProvider>
                  </CardLoaderProvider>
                </CardCacheProvider>
              </SelectedCardsProvider>
            </PricesProvider>
          </CardsProvider>
        </main>
      </SystemTypeProvider>
    </CollectionsProvider>
  );
}
